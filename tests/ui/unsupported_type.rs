use base_web_content::Content;

#[derive(Content)]
struct Request {
    address: std::net::IpAddr,
}

fn main() {}
