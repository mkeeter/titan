use std::convert::TryFrom;
use std::io::{stdout, Read, Write};
use std::sync::{Arc, RwLock};
use std::net::TcpStream;
use anyhow::Result;

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

enum ResponseStatus {
    Input,
    SensitiveInput,
    Success,
    RedirectTemporary,
    RedirectPermanent,
    TemporaryFailure,
    ServerUnavailable,
    CGIError,
    ProxyError,
    SlowDown,
    PermanentFailure,
    NotFound,
    Gone,
    ProxyRequestRefused,
    BadRequest,
    ClientCertificateRequired,
    CertificateNotAuthorized,
    CertificateNotValid,
}

impl TryFrom<u32> for ResponseStatus {
    type Error = ();
    fn try_from(v: u32) -> Result<Self, Self::Error> {
        use ResponseStatus::*;
        Ok(match v {
            10 => Input,
            11 => SensitiveInput,
            20 => Success,
            30 => RedirectTemporary,
            31 => RedirectPermanent,
            40 => TemporaryFailure,
            41 => ServerUnavailable,
            42 => CGIError,
            43 => ProxyError,
            44 => SlowDown,
            50 => PermanentFailure,
            51 => NotFound,
            52 => Gone,
            53 => ProxyRequestRefused,
            59 => BadRequest,
            60 => ClientCertificateRequired,
            61 => CertificateNotAuthorized,
            62 => CertificateNotValid,
            _ => return Err(()),
        })
    }
}

struct ResponseHeader {
    status: ResponseStatus,
    meta: String,
}

fn talk(hostname: &str, page: &str, config: Arc<rustls::ClientConfig>)
    -> Result<Vec<u8>>
{
    let dns_name = webpki::DNSNameRef::try_from_ascii_str(hostname)?;
    let mut sess = rustls::ClientSession::new(&config, dns_name);

    let mut sock = TcpStream::connect(format!("{}:1965", hostname))?;
    let mut tls = rustls::Stream::new(&mut sess, &mut sock);
    tls.write_all(format!("gemini://{}/{}\r\n", hostname, page).as_bytes())?;

    let mut plaintext = Vec::new();
    let rc = tls.read_to_end(&mut plaintext);

    // The server should cleanly close the connection at the end of the
    // message, which returns an error from read_to_end but is actually okay.
    if rc.is_err() {
        let err = rc.unwrap_err();
        if err.kind() != std::io::ErrorKind::ConnectionAborted {
            return Err(err.into());
        }
    }
    Ok(plaintext)
}

fn main() -> Result<()> {
    let dirs = directories::ProjectDirs::from("com", "mkeeter", "titan")
        .ok_or(std::io::Error::new(std::io::ErrorKind::Other,
                                   "Could not get ProjectDirs"))?;
    let db = sled::open(dirs.data_dir())?;
    let certs = db.open_tree("certs")?;

    let mut config = rustls::ClientConfig::new();
    let verifier = GeminiCertificateVerifier { db: RwLock::new(certs) };
    config.dangerous().set_certificate_verifier(Arc::new(verifier));
    let config = Arc::new(config);

    stdout().write_all(&talk("avalos.me", "gemlog/2020-08-22-gemini-makes-me-feel-part-of-something.gmi", config).unwrap()).unwrap();
    Ok(())
}
