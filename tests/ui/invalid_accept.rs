use base_web_content::{Content, Upload};

#[derive(Content)]
struct Profile {
    #[content(accept("image"))]
    avatar: Upload<()>,
}

fn main() {}
