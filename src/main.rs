use std::path::Path;

use regex::Regex;
/*
mostly a port of https://github.com/Henri-J-Norden/py-newgrounds-downloader
*/
use reqwest::{Url, redirect::Policy};

// use clap::{crate_version, Arg, Command as ClapCommand};
fn main() -> anyhow::Result<()> {
    println!("Hello, world!");

	// let mut fi
	get_files("keosni391")?;
	Ok(())
}

// fn download_file()
pub fn get_files(username: &str) -> anyhow::Result<()> {
	/*
	def getFiles(username,folder=".\\",dlType="audio"):
    matches = []
    matches_genres = []
    page_i = 1
    more = True
    while more:
        url = f"https://{username}.newgrounds.com/{dlType}/page/{str(page_i)}"
        print("Fetching '{}'...".format(url))
        req = Request(url, headers={"User-agent":"Mozilla/5.0", "x-requested-with": "XMLHttpRequest"})
        page = str(urlopen(req).read(),encoding="UTF-8")
        matches += re.findall('<a href=.*?newgrounds\.com.*?'+dlType+'.*?listen.*?([0-9]+).*?title\=.*?\"(.+?)\\\\\">', page)
        matches_genres += re.findall('detail-genre.*?(?:\s)+([ \w]+).*?div>', page)
        more = SONGS_ON_PAGE_STRING in page
        page_i += 1
    print("Found {} songs.".format(str(len(matches))))
	print(matches)
    urls = ["https://www.newgrounds.com/audio/download/{}/".format(matches[i][0]) for i in range(len(matches))]
    files = [folder+username+"\\"+matches_genres[i].replace("Song","").strip()+"\\"+matches[i][1]+".mp3" for i in range(len(matches))]
    if not os.path.exists(folder+username):
        os.mkdir(folder+username)
        
    DL(urls,files)
	*/
	// let username = "keosni391";
	let dl_type = "audio";
	let template = format!("https://{username}.newgrounds.com/{dl_type}/page/");
	let mut all_matches = vec![];
	let mut all_matches_genres = vec![];
	let mut i = 1;
	let matches_re = Regex::new(format!("<a href=.*?newgrounds\\.com.*?{dl_type}.*?listen.*?([0-9]+).*?title=.*?\"(.+?)\\\\\">").as_str())?;
	let matches_genres_re = Regex::new("detail-genre.*?(?:\\s)+([ \\w]+).*?div>")?;

	let client = reqwest::blocking::ClientBuilder::new()
	.redirect(Policy::custom(|attempt| {
		let original_url = attempt.previous()[0].clone();
		if attempt.previous().len() != 1 { panic!("multiple previous URLs!"); }
		let redirected_url = attempt.url().clone();
		println!("redirection of {} to {}", original_url, redirected_url);
		// let url_page_number_capture_re = Regex::new()
		// "https://keosni391.newgrounds.com/audio/page/3/" => "https://keosni391.newgrounds.com/audio/page/3/?page=2"
		// this code is super messy and bad and could be refactored to be much better
		let query_string = redirected_url.query().unwrap();
		let query_re = Regex::new("^page=(?P<page_number>\\d+)$").unwrap();
		let query_page_number_cap = query_re.captures(query_string).unwrap().name("page_number").unwrap();
		let original_number_re = Regex::new("page/(?P<page_number>\\d+)/?$").unwrap();
		let original_url_page_number_cap = original_number_re.captures(original_url.path()).unwrap().name("page_number").unwrap();
		if query_page_number_cap.as_str() < original_url_page_number_cap.as_str() {
			attempt.error(format!("Error on redirect of {} to {}", original_url, redirected_url))
		}
		else {
			println!("allowing redirect of {} to {}", original_url, redirected_url);
			attempt.follow()
		}
	}))
	.build()?;
	
	'get_pages: loop {
		let url = Url::parse(format!("{template}{i}").as_str())?;
		println!("fetching {url}");
		let resp = client.get(url).send();
		let content = match resp {
			Ok(resp) => resp.text_with_charset("utf-8")?,
			Err(e) => {
				if e.is_redirect() { break 'get_pages; }
				else { return Err(anyhow::Error::new(e)) }
			}
		};
		// if !content.contains(SONGS_ON_PAGE_STRING) { break 'get_pages; }
		all_matches.extend(matches_re.captures_iter(&content).map(|item| item.get(0).unwrap().as_str().to_owned()));
		all_matches_genres.extend(matches_genres_re.captures_iter(&content).map(|item| item.get(0).unwrap().as_str().to_owned()));
		i += 1;
	}
	all_matches.iter().zip(all_matches_genres.iter()).enumerate()
	.for_each(|(index, (song, genre))| {
		println!("[{index}: {song:?} (genre: {genre:?}");
	});
	Ok(())
}

//#DL(["http://www.newgrounds.com/audio/download/626468"],["file.mp3"])
//def DL(urlList, targetList):
#[allow(unused_variables)]
pub fn download_files(urls: Vec<Url>, files: Vec<&Path>) {
	todo!()
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    // use super::*;

    #[test]
    fn test_download_files() {
        
    }
}