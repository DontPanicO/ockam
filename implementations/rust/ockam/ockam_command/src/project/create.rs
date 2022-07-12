use anyhow::{anyhow, Context};
use clap::Args;
use minicbor::Decoder;
use tracing::debug;

use ockam_api::cloud::project::Project;
use ockam_api::{Response, Status};
use ockam_core::Route;

use crate::node::NodeOpts;
use crate::util::api::CloudOpts;
use crate::util::{api, connect_to, stop_node};
use crate::{CommandGlobalOpts, MessageFormat};

#[derive(Clone, Debug, Args)]
pub struct CreateCommand {
    /// Id of the space the project belongs to.
    #[clap(display_order = 1001)]
    pub space_id: String,

    /// Name of the project.
    #[clap(display_order = 1002)]
    pub project_name: String,

    /// Services enabled for this project.
    #[clap(display_order = 1003)]
    pub services: Vec<String>,
}

impl CreateCommand {
    pub fn run(
        opts: CommandGlobalOpts,
        (cloud_opts, node_opts): (CloudOpts, NodeOpts),
        cmd: CreateCommand,
    ) {
        let cfg = &opts.config;
        let port = match cfg.select_node(&node_opts.api_node) {
            Some(cfg) => cfg.port,
            None => {
                eprintln!("No such node available.  Run `ockam node list` to list available nodes");
                std::process::exit(-1);
            }
        };
        connect_to(port, (opts, cloud_opts, cmd), create);
    }
}

async fn create(
    ctx: ockam::Context,
    (opts, cloud_opts, cmd): (CommandGlobalOpts, CloudOpts, CreateCommand),
    mut base_route: Route,
) -> anyhow::Result<()> {
    let route: Route = base_route.modify().append("_internal.nodeman").into();
    debug!(?cmd, %route, "Sending request");

    let response: Vec<u8> = ctx
        .send_and_receive(route, api::project::create(cmd, cloud_opts)?)
        .await
        .context("Failed to process request")?;
    let mut dec = Decoder::new(&response);
    let header = dec.decode::<Response>()?;
    debug!(?header, "Received response");

    let res = match header.status() {
        Some(Status::Ok) => {
            let body = dec.decode::<Project>()?;
            let output = match opts.global_args.message_format {
                MessageFormat::Plain => "Project created".to_string(),
                MessageFormat::Json => serde_json::to_string(&body)?,
            };
            Ok(output)
        }
        Some(Status::InternalServerError) => {
            let err = dec
                .decode::<String>()
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(anyhow!(
                "An error occurred while processing the request: {err}"
            ))
        }
        _ => Err(anyhow!("Unexpected response received from node")),
    };
    match res {
        Ok(o) => println!("{o}"),
        Err(err) => eprintln!("{err}"),
    };

    stop_node(ctx).await
}
