//! Endpoint MCP `/mcp` (Streamable HTTP, rmcp, stateless + JSON). Lapisan auth memeriksa token
//! bearer OAuth (`services::oauth::authenticate`) dan menaruh `Principal` di extension request;
//! tanpa token valid → 401 dengan `WWW-Authenticate` yang menunjuk metadata resource
//! (RFC 9728). Handler membaca ulang user/role di setiap panggilan (`services::mcp::Session`).
use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::{Next, from_fn_with_state},
    response::{IntoResponse, Response},
    routing::any_service,
};
use loco_rs::{app::AppContext, controller::Routes};
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock,
        GetPromptRequestParams, GetPromptResponse, GetPromptResult, Implementation,
        ListPromptsResult, ListToolsResult, PaginatedRequestParams, Prompt, PromptArgument,
        PromptMessage, Role, ServerCapabilities, ServerConfig, Tool, ToolAnnotations,
    },
    service::RequestContext,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::never::NeverSessionManager,
    },
};

use crate::{
    common::settings::Settings,
    controllers::oauth::www_authenticate,
    services::{
        mcp::{Session, prompts, tools},
        oauth::{self, Principal},
    },
};

const INSTRUCTIONS: &str = "Agent Gate: tiket, workflow stage, artifact, dan gate. Mulai dengan ag_context. \
Draft hasil brainstorming disimpan dengan ag_draft_tickets lalu ditinjau manusia. Pindah stage lewat \
ag_transition (gate dicek server). Bila ragu, tanya manusia dengan ag_ask.";

#[derive(Clone)]
pub struct AgentGateMcp {
    ctx: AppContext,
}

impl AgentGateMcp {
    async fn session(&self, rc: &RequestContext<RoleServer>) -> Result<Session, ErrorData> {
        let principal = rc
            .extensions
            .get::<axum::http::request::Parts>()
            .and_then(|p| p.extensions.get::<Principal>())
            .ok_or_else(|| ErrorData::invalid_request("unauthenticated", None))?;
        Session::load(&self.ctx.db, principal)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
            .ok_or_else(|| ErrorData::invalid_request("access revoked", None))
    }
}

impl ServerHandler for AgentGateMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .build(),
        )
        .with_server_info(
            Implementation::new("agentgate", env!("CARGO_PKG_VERSION")).with_title("Agent Gate"),
        )
        .with_instructions(INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        rc: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        self.session(&rc).await?;
        let list = tools::definitions()
            .into_iter()
            .map(|d| {
                let schema = match d.schema {
                    serde_json::Value::Object(map) => map,
                    _ => serde_json::Map::new(),
                };
                let mut tool = Tool::new(d.name, d.description, Arc::new(schema));
                tool.annotations = Some(ToolAnnotations::new().read_only(d.read_only));
                tool
            })
            .collect();
        Ok(ListToolsResult::with_all_items(list))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        rc: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let session = self.session(&rc).await?;
        let args = request.arguments.map_or(
            serde_json::Value::Object(serde_json::Map::new()),
            serde_json::Value::Object,
        );
        let result = match tools::call(&self.ctx.db, &session, &request.name, args).await {
            Ok(value) => CallToolResult::structured(value),
            Err(e) => CallToolResult::error(vec![ContentBlock::text(e.0)]),
        };
        Ok(result.into())
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        rc: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        let session = self.session(&rc).await?;
        let list = prompts::list(&self.ctx.db, &session)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
            .into_iter()
            .map(|p| {
                let args = p.takes_ticket.then(|| {
                    vec![
                        PromptArgument::new("ticket")
                            .with_description("Kode tiket, mis. KSR-31")
                            .with_required(false),
                    ]
                });
                Prompt::new(p.name, Some(p.description), args).with_title(p.title)
            })
            .collect();
        Ok(ListPromptsResult::with_all_items(list))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        rc: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, ErrorData> {
        let session = self.session(&rc).await?;
        let ticket = request
            .arguments
            .as_ref()
            .and_then(|a| a.get("ticket"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let (description, text) = prompts::render(&self.ctx.db, &session, &request.name, &ticket)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
            .ok_or_else(|| ErrorData::invalid_params("unknown prompt", None))?;
        Ok(
            GetPromptResult::new(vec![PromptMessage::new_text(Role::User, text)])
                .with_description(description)
                .into(),
        )
    }
}

/// Lapisan auth bearer untuk `/mcp`.
async fn require_bearer(State(ctx): State<AppContext>, mut req: Request, next: Next) -> Response {
    let base = Settings::from_ctx(&ctx)
        .map(|s| s.public_base_url)
        .unwrap_or_default();
    let unauthorized = || {
        (
            StatusCode::UNAUTHORIZED,
            www_authenticate(&base),
            axum::Json(serde_json::json!({ "error": "invalid_token" })),
        )
            .into_response()
    };
    let Some(token) = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
        })
        .map(str::trim)
        .filter(|t| !t.is_empty())
    else {
        return unauthorized();
    };
    match oauth::authenticate(&ctx.db, token).await {
        Ok(Some(principal)) => {
            req.extensions_mut().insert(principal);
            next.run(req).await
        }
        Ok(None) => unauthorized(),
        Err(e) => {
            tracing::error!(error = %e, "mcp auth db error");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub fn routes(ctx: &AppContext) -> Routes {
    let handler = AgentGateMcp { ctx: ctx.clone() };
    // Token bearer (bukan cookie), jadi validasi Host/Origin (anti DNS rebinding untuk server
    // lokal) tidak dibutuhkan; host publik dari konfigurasi.
    let config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .disable_allowed_hosts();
    let service: StreamableHttpService<AgentGateMcp, NeverSessionManager> =
        StreamableHttpService::new(move || Ok(handler.clone()), Arc::default(), config);
    Routes::new().add(
        "/mcp",
        any_service(service).layer(from_fn_with_state(ctx.clone(), require_bearer)),
    )
}
