use std::{fs, path};

fn main() {
    //download fonts from git, if it not exist
    let url = "https://wordshub.github.io/free-font/assets/font/%E4%B8%AD%E6%96%87/%E6%96%87%E6%B3%89%E9%A9%BF%E7%B3%BB%E5%88%97/%E6%96%87%E6%B3%89%E9%A9%BF%E6%AD%A3%E9%BB%91.ttc";
    let file = "assets/fonts/文泉驿正黑.ttc";
    // github url:  https://github.com/wordshub/free-font/blob/master/assets/font/%E4%B8%AD%E6%96%87/%E6%96%87%E6%B3%89%E9%A9%BF%E7%B3%BB%E5%88%97/%E6%96%87%E6%B3%89%E9%A9%BF%E5%BE%AE%E7%B1%B3%E9%BB%91.ttc
    // rule:  https://[github_user_id].github.io/[repo_name]/  , no master branch
    // download url: https://wordshub.github.io/free-font/assets/font/%E4%B8%AD%E6%96%87/%E6%96%87%E6%B3%89%E9%A9%BF%E7%B3%BB%E5%88%97/%E6%96%87%E6%B3%89%E9%A9%BF%E6%AD%A3%E9%BB%91.ttc
    // see: https://github.com/orgs/community/discussions/42655#discussioncomment-5669289

    if !path::Path::new(file).exists() {
        // check the path exist, if not, create it.
        {
            let p = path::Path::new(file).parent().unwrap();
            if !p.exists() {
                fs::create_dir_all(p).unwrap();
            }
        }
        let mut file = {
            match fs::File::create(file) {
                Err(e) => {
                    println!("can not create font file:  {}", e);
                    return;
                }
                Ok(f) => f,
            }
        };
        match reqwest::blocking::get(url) {
            Err(e) => {
                println!("can not download font file:  {}", e);
                return;
            }
            Ok(mut response) => {
                if let Err(e) = response.copy_to(&mut file) {
                    println!("can not download font file:  {}", e);
                    return;
                }
            }
        }
    }
}