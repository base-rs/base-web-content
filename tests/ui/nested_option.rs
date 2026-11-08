use base_web_content::Content;

#[derive(Content)]
struct Profile {
    nickname: Option<Option<String>>,
}

fn main() {}
