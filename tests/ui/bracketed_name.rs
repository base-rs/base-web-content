use base_web_content::Content;

#[derive(Content)]
struct Profile {
    #[content(rename = "user[name]")]
    name: String,
}

fn main() {}
