//! Phase 50.1-02 (D-U2): `pane::redact::classify` positive/negative tables and hostile-input guards.
//! Secret shapes follow `50.1-CONTRACT.md` section 9. No test prints a command on success.

use bilterm::pane::redact::{Exclusion, classify};

#[test]
fn secret_shapes_are_classified() {
    let ghp = format!("echo ghp_{}", "a1B2c3D4e5".repeat(4));
    let gho = format!("echo gho_{}", "Z".repeat(25));
    let github_pat = format!("echo github_pat_{}", "x9".repeat(11));
    let aiza = format!("echo AIza{}", "k".repeat(35));
    let sk = "echo sk-abcdefghijklmnopqrstuvwx".to_string();
    let positives: Vec<String> = vec![
        // (a) NAME=value with a secret-ish NAME
        "export API_TOKEN=abc123".into(),
        "DB_PASSWORD=x ./run".into(),
        "PASSWORD=hunter2 make deploy".into(),
        "set -x; SECRET_KEY=abc".into(),
        "env AWS_ACCESS_KEY_ID=abc cmd".into(),
        "GITHUB_TOKEN=\"abc\" git push".into(),
        "export my-api-key=zzz".into(),
        "FOO_CREDENTIALS=file ./x".into(),
        "export PRIVATE_KEY=abc".into(),
        "curl \"https://x/?token=abc\"".into(),
        "export PGPASSWORD='pw' psql".into(),
        // (b) secret flags
        "mycli --password hunter2".into(),
        "mycli --password=hunter2".into(),
        "tool --token abc".into(),
        "tool --api-key=abc".into(),
        "tool --apikey abc".into(),
        "tool --auth-token xyz".into(),
        "tool --client-secret s3cr3t".into(),
        "tool --access-key AKX".into(),
        "gpg --batch --passphrase hunter2 -c f".into(),
        "tool --PASSWORD=Hunter2".into(),
        "tool --secret abc".into(),
        "tool --passwd abc".into(),
        // (c) -p<value> for mysql family and sshpass
        "mysql -u root -phunter2".into(),
        "sshpass -p hunter2 ssh h".into(),
        "mysqldump -u r -pfoo db".into(),
        "sshpass -phunter2 ssh h".into(),
        "mariadb -uroot -psecret".into(),
        "/usr/bin/mysqladmin -pxx status".into(),
        // (d) headers and bearer
        "curl -H \"Authorization: Bearer abcdef\" https://x".into(),
        "curl -H 'X-Api-Key: abcdef' https://x".into(),
        "curl -H \"Proxy-Authorization: Basic Zm9v\" https://x".into(),
        "curl -H \"Authorization:Bearer abc\" x".into(),
        "echo Bearer abcdefghijkl".into(),
        // (e) URL userinfo
        "git clone https://user:pw@host/r.git".into(),
        "curl ftp://u:p@h/f".into(),
        "psql postgres://admin:s3@db.example.com/x".into(),
        // (f) token shapes
        ghp,
        gho,
        github_pat,
        aiza,
        sk,
        "echo AKIAIOSFODNN7EXAMPLE".into(),
        "echo -----BEGIN RSA PRIVATE KEY-----".into(),
        "curl eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0In0.abc".into(),
        "echo xoxb-123456789012-abcdefghij".into(),
        // (g) tool specific
        "openssl enc -aes256 -in f -passin pass:hunter2".into(),
        "openssl req -passout pass:x".into(),
        "echo user:pw | chpasswd".into(),
        "htpasswd -b .htpasswd user pw".into(),
        "htpasswd -bc f u p".into(),
        "echo $X | sudo -S ls".into(),
        "printf '%s\\n' \"$PW\" | sudo -S apt update".into(),
        "echo x|sudo -Sk ls".into(),
    ];
    assert!(positives.len() >= 30, "need at least 30 positives");
    let mut missed = Vec::new();
    for (i, cmd) in positives.iter().enumerate() {
        if classify(cmd) != Some(Exclusion::Secret) {
            // Report the index only, never the text.
            missed.push(i);
        }
    }
    assert!(missed.is_empty(), "positives not classified Secret (indices): {missed:?}");
}

#[test]
fn ordinary_commands_are_not_secret() {
    let negatives = [
        "grep -r password src/",
        "git commit -m \"rotate token\"",
        "echo password",
        "ls",
        "cargo test token_refresh",
        "mysql -u root",
        "ssh -p 2222 host",
        "pwd",
        "sudo ls",
        "ls ~/.ssh",
        "cat /etc/passwd",
        "passwd",
        "git push origin main",
        "docker run -p 8080:80 nginx",
        "curl https://example.com",
        "curl -H \"Accept: application/json\" https://x",
        "git clone git@github.com:foo/bar.git",
        "git clone ssh://git@host/r.git",
        "git clone https://host/r.git",
        "echo token",
        "export PATH=/usr/bin",
        "FOO=bar ./run",
        "tool --password",
        "tool --token --verbose",
        "tool --token-file=/tmp/f",
        "tool --no-password",
        "cargo build --release",
        "echo $TOKEN",
        "htpasswd file user",
        "htpasswd -B f u",
        "gpg --list-keys",
        "openssl x509 -in f -noout",
        "openssl enc -passin env:PW",
        "echo hello | sudo tee f",
        "sudo -S ls",
        "echo task-assessment-and-something-long-name",
        "ps aux | grep sshpass",
        "mysql -h host -P 3306",
        "TOKEN= ./run",
        "export TOKEN=\"\"",
        "echo sk-short",
        "echo AKIA123",
        "echo xoxb-",
        "echo eyJabc.def",
        "echo -----BEGIN CERTIFICATE-----",
        "echo Bearer",
        "curl -H \"Authorization:\"",
        "sshpass -f pwfile ssh h",
    ];
    assert!(negatives.len() >= 30, "need at least 30 negatives");
    let flagged: Vec<usize> = negatives
        .iter()
        .enumerate()
        .filter(|(_, c)| classify(c).is_some())
        .map(|(i, _)| i)
        .collect();
    assert!(flagged.is_empty(), "negatives wrongly excluded (indices): {flagged:?}");
}

#[test]
fn blank_and_colon() {
    assert_eq!(classify(""), Some(Exclusion::Empty));
    assert_eq!(classify("   "), Some(Exclusion::Empty));
    assert_eq!(classify("\t\n"), Some(Exclusion::Empty));
    assert_eq!(classify(":find agent x"), Some(Exclusion::Colon));
    assert_eq!(classify("  :q"), Some(Exclusion::Colon));
    assert_eq!(classify(":set api_token=abc"), Some(Exclusion::Secret));
}

#[test]
fn hostile_input_is_safe() {
    let mib = 1024 * 1024;
    let pairs = "a=".repeat(mib / 2);
    assert_eq!(classify(&pairs), None);

    let emoji = "\u{1F600}".repeat(mib / 4);
    assert_eq!(classify(&emoji), None);

    let spaced = "a ".repeat(mib / 2);
    assert_eq!(classify(&spaced), None);

    let dashes_p = "-p".repeat(mib / 2);
    assert_eq!(classify(&dashes_p), None);

    let jwtish = "eyJ-".repeat(mib / 4);
    assert_eq!(classify(&jwtish), None);

    let urlish = "a://".repeat(mib / 4);
    assert_eq!(classify(&urlish), None);

    let long_name = format!("{}=v", "x".repeat(mib));
    assert_eq!(classify(&long_name), None);

    let _ = classify("a\0b=c\u{1b}[31m x");
    let _ = classify("\u{202E}");
    let _ = classify("\u{202E}password=x");
    let _ = classify("é=ü --passwordé=ü ñ://é:ü@ö");
    assert_eq!(classify("PASSWORD=x\0"), Some(Exclusion::Secret));
}
