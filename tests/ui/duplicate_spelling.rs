use base_web_content::FieldValue;

#[derive(FieldValue)]
enum Role {
    Admin,
    #[content(rename = "Admin")]
    Administrator,
}

fn main() {}
