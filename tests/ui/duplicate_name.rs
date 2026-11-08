use base_web_content::Content;

#[derive(Content)]
struct Signup {
    email: String,
    #[content(rename = "email")]
    confirmation: String,
}

fn main() {}
