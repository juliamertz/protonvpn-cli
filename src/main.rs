use protonvpn_rs::{cli, config};

fn main() -> anyhow::Result<()> {
    let matches = cli::init().get_matches();

    // Most subcommands interface with the deamon's socket. this requires root privilleges so we
    // might as well escalate right from the start to prevent replaying the program state.
    elevate::escalate_if_needed().expect("to escalate");
    config::init(&matches)?;

    match matches.subcommand() {
        Some(("connect", args)) => cli::handle_connect_subcommand(args),
        Some(("disconnect", args)) => cli::handle_disconnect_subcommand(args),
        Some(("service", args)) => cli::handle_service_subcommand(args),
        Some(("query", args)) => cli::handle_query_subcommand(args),
        Some(("status", args)) => cli::handle_status_subcommand(args),
        Some(("config", args)) => cli::handle_config_subcommand(args),
        Some(("killswitch", args)) => cli::handle_killswitch_subcommand(args),
        _ => unimplemented!(),
    }?;

    Ok(())
}
