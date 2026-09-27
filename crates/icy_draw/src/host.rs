use icy_engine::TextPane;
use icy_engine_edit::collaboration::{AutosaveConfig, Block, ServerConfig};
use std::{path::PathBuf, time::Duration};

#[derive(clap::Args)]
pub struct HostArgs {
    #[arg(short, long, default_value_t = 8000)]
    pub port: u16,
    #[arg(short, long, default_value = "0.0.0.0")]
    pub bind: std::net::IpAddr,
    #[arg(long)]
    pub password: Option<String>,
    #[arg(long, default_value_t = 0)]
    pub max_users: usize,
    pub file: Option<PathBuf>,
    #[arg(long)]
    pub backup_folder: Option<PathBuf>,
    #[arg(long, default_value_t = 60)]
    pub interval: u64,
}

impl HostArgs {
    pub fn config(&self) -> anyhow::Result<ServerConfig> {
        let seconds = self.interval.checked_mul(60).ok_or_else(|| anyhow::anyhow!("Autosave interval is too large"))?;
        let mut config = ServerConfig {
            bind_addr: std::net::SocketAddr::new(self.bind, self.port),
            password: self.password.clone().unwrap_or_default(),
            max_users: self.max_users,
            autosave: AutosaveConfig {
                backup_folder: self.backup_folder.clone().unwrap_or_else(|| PathBuf::from(".")),
                interval: (seconds != 0).then(|| Duration::from_secs(seconds)),
                ..Default::default()
            },
            ..Default::default()
        };
        if let Some(path) = &self.file {
            let document = crate::document::Document::load(path).map_err(anyhow::Error::msg)?;
            document.with_state(|state| {
                let buffer = state.get_buffer();
                config.columns = buffer.width() as u32;
                config.rows = buffer.height() as u32;
                config.initial_document = Some(
                    (0..buffer.width())
                        .map(|column| {
                            (0..buffer.height())
                                .map(|row| {
                                    let character = buffer.char_at((column, row).into());
                                    let attribute = character.attribute.as_u8(buffer.ice_mode);
                                    Block {
                                        code: character.ch as u32,
                                        fg: attribute & 15,
                                        bg: attribute >> 4,
                                    }
                                })
                                .collect()
                        })
                        .collect(),
                );
                config.ice_colors = buffer.ice_mode == icy_engine::IceMode::Ice;
                config.use_9px_font = buffer.use_letter_spacing();
                config.font_name = buffer.font(0).map_or_else(|| "IBM VGA".into(), |font| font.name().into());
                config.sauce = state.get_sauce_meta().clone();
                for (index, color) in config.palette.iter_mut().enumerate() {
                    let (red, green, blue) = buffer.palette.rgb(index as u32);
                    *color = [red, green, blue];
                }
            });
        }
        Ok(config)
    }

    pub fn run(self) -> anyhow::Result<()> {
        let config = self.config()?;
        tokio::runtime::Runtime::new()?.block_on(icy_engine_edit::collaboration::run_server(config))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        host: HostArgs,
    }

    #[test]
    fn cli_defaults_and_document_configuration() {
        let defaults = Cli::try_parse_from(["host"]).unwrap().host.config().unwrap();
        assert_eq!(defaults.bind_addr.to_string(), "0.0.0.0:8000");
        assert_eq!(defaults.autosave.interval, Some(Duration::from_secs(3600)));
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("host.icy");
        let mut document = crate::document::Document::new(icy_engine::Size::new(40, 12));
        document.type_text("HELLO").unwrap();
        document.with_state(|state| state.set_use_letter_spacing(true)).unwrap();
        document.save(&path, false).unwrap();
        let config = Cli::try_parse_from(["host", path.to_str().unwrap(), "--bind", "::1", "--interval", "0"])
            .unwrap()
            .host
            .config()
            .unwrap();
        assert_eq!((config.columns, config.rows), (40, 12));
        assert_eq!(config.initial_document.unwrap()[0][0].code, 'H' as u32);
        assert!(config.use_9px_font);
        assert_eq!(config.autosave.interval, None);
    }
}
