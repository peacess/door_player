use std::path::PathBuf;
use std::{fs, mem};

pub fn get_font() -> Result<PathBuf, anyhow::Error> {
    let file = "assets/fonts/文泉驿正黑.ttc";
    //download fonts from git, if it not exist
    let url = "https://wordshub.github.io/free-font/assets/font/%E4%B8%AD%E6%96%87/%E6%96%87%E6%B3%89%E9%A9%BF%E7%B3%BB%E5%88%97/%E6%96%87%E6%B3%89%E9%A9%BF%E6%AD%A3%E9%BB%91.ttc";
    // let url = "https://raw.githubusercontent.com/wordshub/free-font/master/assets/font/%E4%B8%AD%E6%96%87/%E6%96%87%E6%B3%89%E9%A9%BF%E7%B3%BB%E5%88%97/%E6%96%87%E6%B3%89%E9%A9%BF%E6%AD%A3%E9%BB%91.ttc";
    // github url:  https://github.com/wordshub/free-font/blob/master/assets/font/%E4%B8%AD%E6%96%87/%E6%96%87%E6%B3%89%E9%A9%BF%E7%B3%BB%E5%88%97/%E6%96%87%E6%B3%89%E9%A9%BF%E5%BE%AE%E7%B1%B3%E9%BB%91.ttc
    // rule:  https://[github_user_id].github.io/[repo_name]/  , no master branch
    // download url: https://wordshub.github.io/free-font/assets/font/%E4%B8%AD%E6%96%87/%E6%96%87%E6%B3%89%E9%A9%BF%E7%B3%BB%E5%88%97/%E6%96%87%E6%B3%89%E9%A9%BF%E6%AD%A3%E9%BB%91.ttc
    // see: https://github.com/orgs/community/discussions/42655#discussioncomment-5669289

    let mut file_path = PathBuf::from(file);
    if !file_path.exists() {
        let exe_path = {
            let temp = std::env::current_exe()?;
            temp.parent().expect("").to_owned()
        };

        let exe_path = exe_path.join(file);
        if !exe_path.exists() {
            {
                let p = exe_path.parent().expect("");
                if !p.exists() {
                    fs::create_dir_all(p)?;
                }
            }
            let mut response = reqwest::blocking::get(url)?;
            let mut exe_file = fs::File::create(exe_path.clone())?;
            if let Err(e) = response.copy_to(&mut exe_file) {
                mem::drop(exe_file);
                let _ = fs::remove_file(exe_path);
                return Err(e.into());
            }
        }
        file_path = exe_path
    }

    Ok(file_path)
}
