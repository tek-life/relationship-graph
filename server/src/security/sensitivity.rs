pub fn display_name(name: &str, aliases: &[String], sensitivity_level: &str, revealed: bool) -> String {
    if sensitivity_level == "low" || revealed {
        return name.to_string();
    }

    aliases
        .first()
        .cloned()
        .unwrap_or_else(|| "高敏感联系人".to_string())
}

pub fn requires_reveal(sensitivity_level: &str) -> bool {
    sensitivity_level == "high"
}
