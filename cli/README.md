# WTUMCL CLI

A command-line client for the WTUMCL GUI app, connected through its MCP server.

> Adapted from the upstream CLI guide in `UNIkeEN/SJMCL` (`888857c7`, *docs(cli): reorganize the CLI introduction and usage guide*) and rewritten for this fork, whose CLI exposes the generic MCP tool command only.

## Build

From the repository root:

```bash
cargo build --release --manifest-path cli/Cargo.toml
```

The binary is `cli/target/release/sjmcl-cli` (`sjmcl-cli.exe` on Windows). The examples below assume it is on your `PATH`.

## Connect to the GUI app

Enable **Launcher MCP Service** in WTUMCL's **Intelligence** settings. The CLI connects to `127.0.0.1:18970` by default. For a different port, place `-p <port>` (or `--port <port>`) before the command:

```bash
sjmcl-cli -p 18971 retrieve_launcher_config
```

When the first connection fails, the CLI opens the `wtumcl://run-silently` deeplink to start the launcher in the background and retries for up to 5 seconds. If the connection still fails, it prints a hint about enabling the MCP server and exits with status `1`.

## Commands

### MCP Tools

MCP tools cover launcher configuration, accounts, instances, launching, resources, news, extensions and more.

Call a tool by its name, with arguments as a single JSON object:

```bash
sjmcl-cli retrieve_launcher_config
sjmcl-cli retrieve_instance_list
sjmcl-cli launch_instance '{"instance_id":"<instance-id>"}'
```

`--help` lists the available tools and their parameters when the launcher is reachable; otherwise it prints the basic usage and the connection hint:

```bash
sjmcl-cli --help
```

The CLI prints the tool response and returns. For tools that queue background tasks, the completion must be checked separately with the matching query tool. A tool that reports an MCP error makes the CLI exit with status `1`.

### Progress and exit status

A progress spinner is drawn on stderr only when stderr is a terminal, so redirected output stays clean. Errors are printed on stderr as `Error: <message>` and the process exits with status `1`; success exits with `0`.

### Shortcuts

The upstream CLI also ships `install`, `download` and `downloads` shortcuts. This fork's CLI does not implement them yet, so use the MCP tools above instead.
