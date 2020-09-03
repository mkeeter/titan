use std::io::{stdout, Read, Write};
use std::sync::Arc;
use std::net::TcpStream;

struct GeminiCertificateVerifier { }

impl rustls::ServerCertVerifier for GeminiCertificateVerifier {
    fn verify_server_cert(&self,
                          roots: &rustls::RootCertStore,
                          presented_certs: &[rustls::Certificate],
                          dns_name: webpki::DNSNameRef<'_>,
                          ocsp_response: &[u8])
        -> Result<rustls::ServerCertVerified, rustls::TLSError>
    {
        Ok(rustls::ServerCertVerified::assertion())
    }
}

fn main() {
    let mut config = rustls::ClientConfig::new();
    config.dangerous().set_certificate_verifier(Arc::new(GeminiCertificateVerifier { }));

    let dns_name = webpki::DNSNameRef::try_from_ascii_str("gemini.circumlunar.space").unwrap();
    let mut sess = rustls::ClientSession::new(&Arc::new(config), dns_name);

    let mut sock = TcpStream::connect("gemini.circumlunar.space:1965")
        .expect("Couldn't connect to the server...");
    let mut tls = rustls::Stream::new(&mut sess, &mut sock);
    tls.write("gemini://gemini.circumlunar.space/docs/\r\n".as_bytes()).unwrap();

    let mut plaintext = Vec::new();
    print!("{:?}", tls.read_to_end(&mut plaintext));
    stdout().write_all(&plaintext).unwrap();
}
