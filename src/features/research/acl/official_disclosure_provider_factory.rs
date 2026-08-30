use crate::config::AppConfig;
use crate::features::research::application::official_disclosure_provider::OfficialDisclosureProvider;
use crate::features::research::infrastructure::sec_edgar_official_disclosure_provider::SecEdgarOfficialDisclosureProvider;
use std::path::Path;

/// Official Disclosure provider の infrastructure 実装を application から隠蔽する。
#[allow(dead_code)]
pub(crate) fn build_official_disclosure_provider(
    app_config: &AppConfig,
    save_dir: &Path,
) -> Result<Box<dyn OfficialDisclosureProvider>, String> {
    let user_agent = app_config
        .sec
        .as_ref()
        .map(|config| config.user_agent.clone());
    let cache_path = save_dir
        .join("corporate_event")
        .join("sec_company_identity_cache.json");
    SecEdgarOfficialDisclosureProvider::new(user_agent, Some(cache_path))
        .map(|provider| Box::new(provider) as Box<dyn OfficialDisclosureProvider>)
}

#[cfg(test)]
mod tests {
    use super::build_official_disclosure_provider;
    use crate::config::AppConfig;
    use tempfile::tempdir;

    #[test]
    fn factory_builds_without_starting_a_network_request() {
        let config = AppConfig::load("config.toml").expect("repository config should parse");
        let directory = tempdir().expect("temporary cache directory should be available");

        let provider = build_official_disclosure_provider(&config, directory.path());

        assert!(provider.is_ok());
    }
}
