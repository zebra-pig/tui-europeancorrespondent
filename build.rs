fn main() {
    // Read .env file and set values as compile-time environment variables
    if let Ok(contents) = std::fs::read_to_string(".env") {
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            if let Some((key, value)) = line.split_once('=') {
                println!("cargo:rustc-env={}={}", key.trim(), value.trim());
            }
        }
    }

    // Also forward EC_API_KEY from environment (for CI builds)
    if let Ok(key) = std::env::var("EC_API_KEY") {
        println!("cargo:rustc-env=EC_API_KEY={}", key);
    }

    println!("cargo:rerun-if-changed=.env");
}
