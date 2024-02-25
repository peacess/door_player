use std::fs;
use std::path::PathBuf;

pub fn get_font() -> Result<PathBuf, anyhow::Error> {
    let file = "assets/fonts/文泉驿正黑.ttc";
    //download fonts from git, if it not exist
    let url = "https://wordshub.github.io/free-font/assets/font/%E4%B8%AD%E6%96%87/%E6%96%87%E6%B3%89%E9%A9%BF%E7%B3%BB%E5%88%97/%E6%96%87%E6%B3%89%E9%A9%BF%E6%AD%A3%E9%BB%91.ttc";
    // github url:  https://github.com/wordshub/free-font/blob/master/assets/font/%E4%B8%AD%E6%96%87/%E6%96%87%E6%B3%89%E9%A9%BF%E7%B3%BB%E5%88%97/%E6%96%87%E6%B3%89%E9%A9%BF%E5%BE%AE%E7%B1%B3%E9%BB%91.ttc
    // rule:  https://[github_user_id].github.io/[repo_name]/  , no master branch
    // download url: https://wordshub.github.io/free-font/assets/font/%E4%B8%AD%E6%96%87/%E6%96%87%E6%B3%89%E9%A9%BF%E7%B3%BB%E5%88%97/%E6%96%87%E6%B3%89%E9%A9%BF%E6%AD%A3%E9%BB%91.ttc
    // see: https://github.com/orgs/community/discussions/42655#discussioncomment-5669289

    let exe_path = {
        let temp = std::env::current_exe()?;
        temp.parent().expect("").to_owned()
    };

    let full_file = exe_path.join(file);

    if !full_file.exists() {
        // check the path exist, if not, create it.
        {
            let p = full_file.parent().expect("");
            if !p.exists() {
                fs::create_dir_all(p)?;
            }
        }
        let mut file = fs::File::create(file)?;
        let mut response = reqwest::blocking::get(url)?;
        response.copy_to(&mut file)?;
    }

    Ok(full_file)
}