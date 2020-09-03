use std::io::{stdout, Read, Write};
use std::sync::{Arc, RwLock};
use std::net::TcpStream;

struct GeminiCertificateVerifier {
    db: RwLock<sled::Tree>
}

impl rustls::ServerCertVerifier for GeminiCertificateVerifier {
    fn verify_server_cert(&self,
                          _roots: &rustls::RootCertStore,
                          presented_certs: &[rustls::Certificate],
                          dns_name: webpki::DNSNameRef<'_>,
                          _ocsp_response: &[u8])
        -> Result<rustls::ServerCertVerified, rustls::TLSError>
    {
        use rustls::{TLSError, ServerCertVerified};

        if presented_certs.is_empty() {
            return Err(TLSError::NoCertificatesPresented)
        }

        let dns_name = dns_name.to_owned();
        let d : &str = AsRef::<str>::as_ref(&dns_name);
        let r = self.db.read().unwrap().get(&d)
            .map_err(|e| TLSError::General(e.to_string()))?;

        if let Some(c) = r {
            if c == presented_certs[0].as_ref() {
                Ok(ServerCertVerified::assertion())
            } else {
                Err(TLSError::WebPKIError(webpki::Error::CertNotValidForName))
            }
        } else {
            self.db.write().unwrap()
                .insert(d, presented_certs[0].as_ref())
                .map_err(|e| TLSError::General(e.to_string()))?;
            Ok(ServerCertVerified::assertion())
        }
    }
}

fn talk(hostname: &str, page: &str, config: Arc<rustls::ClientConfig>) {
    let dns_name = webpki::DNSNameRef::try_from_ascii_str(hostname).unwrap();
    let mut sess = rustls::ClientSession::new(&config, dns_name);

    let mut sock = TcpStream::connect("gemini.circumlunar.space:1965")
        .expect("Couldn't connect to the server...");
    let mut tls = rustls::Stream::new(&mut sess, &mut sock);
    tls.write_all(format!("gemini://{}/{}/\r\n", hostname, page).as_bytes())
        .expect("Could not write initial message");

    let mut plaintext = Vec::new();
    println!("{:?}", tls.read_to_end(&mut plaintext));
    stdout().write_all(&plaintext).unwrap();
}

fn main() {
    let dirs = directories::ProjectDirs::from("com", "mkeeter", "titan")
        .expect("Could not get project dirs");
    let db = sled::open(dirs.data_dir())
        .expect("Could not open db");
    let certs = db.open_tree("certs")
        .expect("Could not open certs tree");

    let mut config = rustls::ClientConfig::new();
    let verifier = GeminiCertificateVerifier { db: RwLock::new(certs) };
    config.dangerous().set_certificate_verifier(Arc::new(verifier));
    let config = Arc::new(config);

    talk("gemini.circumlunar.space", "", config);
}
