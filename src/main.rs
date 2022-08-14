#![feature(type_name_of_val)]
use std::{
	io::Write,
	path::{Path, PathBuf},
	time::Duration,
};

use backoff::{retry, ExponentialBackoff};
use derive_builder::Builder;
use ego_tree::iter::Edge;
use regex::Regex;
/*
mostly a port of https://github.com/Henri-J-Norden/py-newgrounds-downloader
*/
use reqwest::{redirect::Policy, StatusCode, Url};
use scraper::{node::Node, Html, Selector};
use serde::{Deserialize, Serialize};
// use time::Date;

// use clap::{crate_version, Arg, Command as ClapCommand};
fn main() -> anyhow::Result<()> {
	println!("Start of newgrounds-audio-downloader");

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
	let dl_type = "audio";
	let dl_directory = "/mnt/ehdd/DH/music/artists/K-391/newgrounds";

	let template = format!("https://{username}.newgrounds.com/{dl_type}/page/");
	let mut all_matches = vec![];
	//TODO: possibly scrape the songs' listen pages instead (probably more robust, and likely additional information can be retrieved from there) (TODO: check if likes, downloads, upload date, etc are already archived by us)
	let mut all_matches_genres = vec![];
	let mut i = 1;
	let matches_re = Regex::new(
		format!(
			"<a href=.*?newgrounds\\.com.*?{dl_type}.*?listen.*?([0-9]+)\" class=\"item-audiosubmission \">"
		)
		.as_str(),
	)?;
	let matches_genres_re = Regex::new("detail-genre.*?(?:\\s)+([ \\w]+).*?div>")?;

	let client = reqwest::blocking::ClientBuilder::new()
		.redirect(Policy::custom(|attempt| {
			let original_url = attempt.previous()[0].clone();
			if attempt.previous().len() != 1 {
				panic!("multiple previous URLs!");
			}
			let redirected_url = attempt.url().clone();
			println!("redirection of {} to {}", original_url, redirected_url);
			// let url_page_number_capture_re = Regex::new()
			// "https://keosni391.newgrounds.com/audio/page/3/" => "https://keosni391.newgrounds.com/audio/page/3/?page=2"
			// this code is super messy and bad and could be refactored to be much better
			let query_string = redirected_url.query().unwrap();
			let query_re = Regex::new("^page=(?P<page_number>\\d+)$").unwrap();
			let query_page_number_cap = query_re
				.captures(query_string)
				.unwrap()
				.name("page_number")
				.unwrap();
			let original_number_re = Regex::new("page/(?P<page_number>\\d+)/?$").unwrap();
			let original_url_page_number_cap = original_number_re
				.captures(original_url.path())
				.unwrap()
				.name("page_number")
				.unwrap();
			if query_page_number_cap.as_str() < original_url_page_number_cap.as_str() {
				attempt.error(format!(
					"Error on redirect of {} to {}",
					original_url, redirected_url
				))
			} else {
				println!(
					"allowing redirect of {} to {}",
					original_url, redirected_url
				);
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
				if e.is_redirect() {
					break 'get_pages;
				} else {
					return Err(anyhow::Error::new(e));
				}
			}
		};

		all_matches.extend(
			matches_re
				.captures_iter(&content)
				.map(|item| item.get(0).unwrap().as_str().to_owned()),
		);
		all_matches_genres.extend(
			matches_genres_re
				.captures_iter(&content)
				.map(|item| item.get(0).unwrap().as_str().to_owned()),
		);
		i += 1;
	}
	// assert!(all_matches.len() == all_matches_genres.len());
	println!("{} songs found", all_matches.len());
	// all_matches
	// 	.iter()
	// 	.zip(all_matches_genres.iter())
	// 	.enumerate()
	// 	.for_each(|(index, (song, genre))| {
	// 		println!("[{index}: {song:?} (genre: {genre:?}");
	// 	});
	let song_ids = all_matches
		.iter()
		.map(|item| {
			//example_usage: `<a href="https://www.newgrounds.com/audio/listen/429941" class="item-audiosubmission ">` => `https://www.newgrounds.com/audio/listen/429941`
			let url_extraction_regex =
				Regex::new(r#"^<a href="https://www.newgrounds.com/audio/listen/(?P<id>\d+)"#)
					.unwrap();
			url_extraction_regex
				.captures(item)
				.unwrap()
				.name("id")
				.unwrap()
				.as_str()
		})
		.collect::<Vec<_>>();
	// let urls = all_matches...
	let files = song_ids
		.iter()
		.map(|id| Path::new(dl_directory).join(id))
		.collect::<Vec<_>>();

	let urls = song_ids
		.iter()
		.map(|id| Url::parse(&format!("http://www.newgrounds.com/audio/download/{id}")).unwrap())
		.collect();

	//TODO: download metadata as well
	let metadatas = get_metadata_for_ids(&song_ids)?;
	let mut metadata_location = PathBuf::new();
	metadata_location.push(dl_directory);
	metadata_location.push("metadata.json");
	let metadata_file = std::fs::File::create(metadata_location)?;
	serde_json::to_writer_pretty(metadata_file, &metadatas)?;
	download_files(urls, files)?;
	Ok(())
}

/// example usage:
/// ```rs
/// download_files([Url::parse("http://www.newgrounds.com/audio/download/626468").unwrap()],[Path::new("file.mp3").to_path_buf]);
/// ```
#[allow(unused_variables)]
//TODO: make async
pub fn download_files(urls: Vec<Url>, mut files: Vec<PathBuf>) -> anyhow::Result<()> {
	assert!(urls.len() == files.len());
	// urls.iter()
	// 	.zip(files.iter())
	// 	.for_each(|(url, file)| println!("{url} | {file:?}"));
	//TODO: remove file field from program
	urls.into_iter()
		.zip(files.iter_mut())
		.for_each(|(url, file)| {
			let backoff = ExponentialBackoff::default();
			let instant = std::time::Instant::now();
			let op = || {
				println!(
					"backoff due to rate-limiting for url {url}: {:?} seconds",
					instant.elapsed().as_secs()
				);
				// println!("Fetching {}", url);
				// let mut resp = reqwest::blocking::get(url)?;

				// let mut content = String::new();
				// let _ = resp.read_to_string(&mut content);
				// Ok(content)

				let response = reqwest::blocking::get(url.clone())
					.map_err(backoff::Error::transient)
					.unwrap();
				if response.status() == StatusCode::TOO_MANY_REQUESTS {
					return Err(backoff::Error::Transient {
						err: response.status().canonical_reason().unwrap(),
						retry_after: match response.headers().get("Retry-After") {
							Some(time) => {
								if let Ok(time_str) = time.to_str() {
									if let Ok(seconds) = time_str.parse::<u64>() {
										Some(Duration::from_secs(seconds))
									} else if let Ok(systemtime) =
										httpdate::parse_http_date(time_str)
									{
										println!(
											"got SystemTime \"{}\" for {}",
											httpdate::fmt_http_date(systemtime),
											&url
										);
										systemtime
											.duration_since(std::time::SystemTime::UNIX_EPOCH)
											.ok()
									} else {
										None
									}
								} else {
									None
								}
							}
							None => None,
						},
					});
				}
				Ok(response)
			};

			let response = retry(backoff, op).unwrap();

			let headers = response.headers();
			let file_name = headers.get("content-disposition");
			let file_name = file_name
				.unwrap_or_else(|| panic!("no content-disposition header for {}", response.url()))
				.to_str()
				.unwrap();
			let file_name_re = Regex::new(r#"^attachment; filename="(?P<file_name>.+)"$"#).unwrap();
			let file_name = file_name_re
				.captures(file_name)
				.unwrap_or_else(|| panic!("failed to extract file name from {file_name}"))
				.name("file_name")
				.unwrap()
				.as_str();
			file.pop();
			file.push(file_name);

			match Path::new(file).try_exists() {
				Ok(is_not_a_broken_symlink) => {
					if is_not_a_broken_symlink {
						let response_bytes = response.bytes().unwrap();
						let existing_file_bytes = std::fs::read(file.clone()).unwrap();
						if (response_bytes.len() != existing_file_bytes.len())
							|| (seahash::hash(&existing_file_bytes)
								!= seahash::hash(&response_bytes))
						{
							panic!("different size or different hashes for existing file ({file:?}) and the new download");
						} else {
							println!("identical size and hashes for url {url}! Skipping");
						}
					} else {
						panic!("broken symlink for file {file:?}")
					}
				}
				Err(e) => {
					if std::io::ErrorKind::NotFound == e.kind() {
						std::fs::File::create(file.clone())
							.unwrap()
							.write_all(&response.bytes().unwrap())
							.unwrap();
						println!("Successfully downloaded {url} to {file:?}");
					} else {
						panic!("{e} {file:?}");
					}
				}
			}
		});
	// println!("{:?}", reqwest::blocking::get(urls[0].clone()).unwrap().headers());
	// let response = reqwest::blocking::get(url)?;
	// let mut file = std::fs::File::create(file_name)?;
	// let mut content =  Cursor::new(response.bytes().await?);
	// std::io::copy(&mut content, &mut file)?;
	Ok(())
}

pub fn get_metadata_for_ids(ids: &[&str]) -> anyhow::Result<Vec<NewGroundsAudioMetadata>> {
	macro_rules! extract_node_from_edge {
		($edge:ident) => {
			match $edge {
				Edge::Open(node) => node,
				Edge::Close(node) => node,
			}
		};
	}
	let base_metadata_url = Url::parse("https://www.newgrounds.com/audio/listen/")?;
	Ok(ids
		.iter()
		.map(|id| -> anyhow::Result<NewGroundsAudioMetadata> {
			let url = base_metadata_url.clone().join(id)?;
			println!("{}", url);
			let response = reqwest::blocking::get(url)?;
			let html = Html::parse_document(&response.text()?);
			let mut builder = NewGroundsAudioMetadataBuilder::default();
			let selector = Selector::parse("#sidestats").unwrap();
			// let stats_selectors = html.select(&selector);
			// println!("found {} matching selectors", stats_selectors.count());
			let mut stats_selector = html.select(&selector);
			let sidestats_div = stats_selector.next().unwrap();
			assert!(stats_selector.next() == None);
			// let dt_selector = Selector::parse("dt").unwrap();
			// let dd_selector = Selector::parse("dd").unwrap();
			let whitespace_re = Regex::new(r"^\s+$").unwrap();
			let pending_score_re = Regex::new(r"^\s*Waiting for \d+ more votes?\s*$").unwrap();
			let mut sidestats_iter = sidestats_div.traverse();
			#[cfg(not(feature = "a"))]
			{
				'main: while let Some(child) = sidestats_iter.next() {
					// both variants contain the same stuff
					let node = extract_node_from_edge!(child);
					// if let Some(raw_listens) = node.value()
					// .as_element().unwrap()
					// .select(&dd_selector).next() {
					// 	println!("{raw_listens}");
					// }

					let value = node.value();
					if let Node::Element(element) = value {
						if element.name() == "dt" {
							if let Some(child) = sidestats_iter.next() {
								let node = extract_node_from_edge!(child);
								let value = node.value();
								if let Node::Text(text) = value {
									match &**text {
										"Listens" => {
											sidestats_iter.next(); // Listens clone (ending/closing part - applies to below too)
											sidestats_iter.next(); // dt clone
											sidestats_iter.next(); // whitespace
											sidestats_iter.next(); // whitespace clone
											if let Some(child) = sidestats_iter.next() {
												let node = extract_node_from_edge!(child);
												let value = node.value();
												if let Node::Element(element) = value {
													if element.name() == "dd" {
														if let Some(child) = sidestats_iter.next() {
															let node =
																extract_node_from_edge!(child);
															let value = node.value();
															if let Node::Text(text) = value {
																println!("text: {:?}", text);
																let text =
																	(**text).replace(',', "");
																let listens =
																	text.parse::<u64>().unwrap();
																builder.listens(listens);
															} else {
																panic!()
															}
														}
													}
												}
											}
										}
										"Faves:" => {
											sidestats_iter.next();
											sidestats_iter.next();
											while let Some(child) = sidestats_iter.next() {
												let node = extract_node_from_edge!(child);
												let value = node.value();
												if let Node::Text(text) = value {
													let text = (**text).replace(',', "");
													if let Ok(faves) = text.parse::<u64>() {
														builder.faves(faves);
														continue 'main;
													}
												}
											}
										}
										"Downloads" => {
											sidestats_iter.next();
											sidestats_iter.next();
											while let Some(child) = sidestats_iter.next() {
												let node = extract_node_from_edge!(child);
												let value = node.value();
												if let Node::Text(text) = value {
													let text = (**text).replace(',', "");
													if let Ok(downloads) = text.parse::<u64>() {
														builder.downloads(downloads);
														continue 'main;
													}
												}
											}
										}
										"Votes" => {
											sidestats_iter.next();
											sidestats_iter.next();
											while let Some(child) = sidestats_iter.next() {
												let node = extract_node_from_edge!(child);
												let value = node.value();
												if let Node::Text(text) = value {
													let text = (**text).replace(',', "");
													if let Ok(votes) = text.parse::<u64>() {
														builder.votes(Some(votes));
														continue 'main;
													}
												}
											}
										}
										"Score" => {
											sidestats_iter.next();
											sidestats_iter.next();
											while let Some(child) = sidestats_iter.next() {
												let node = extract_node_from_edge!(child);
												let value = node.value();
												if let Node::Text(text) = value {
													let text = (**text).replace(',', "");
													if let Ok(score) = text.parse::<f64>() {
														builder.score(ScoreType::Score(score));
														continue 'main;
													} else if pending_score_re.is_match(&text) {
														builder.votes(None);
														builder.score(ScoreType::Waiting(text));
														builder.votes(None);
														continue 'main;
													}
												}
											}
										}
										"Uploaded" => {
											sidestats_iter.next();
											sidestats_iter.next();
											sidestats_iter.next();
											let mut upload_date = String::new();
											let mut c: u8 = 0;
											while let Some(child) = sidestats_iter.next() {
												sidestats_iter.next();
												let node = extract_node_from_edge!(child);
												let value = node.value();
												if let Node::Text(text) = value {
													let text = &(**text);
													if text.is_empty() {
														continue;
													}
													if whitespace_re.is_match(text) {
														continue;
													}
													if c == 1 {
														upload_date.push(' ');
													}
													upload_date.push_str(text);
													c += 1;
												} else if c > 1 {
													println!("upload_date: {}", &upload_date);
													builder.uploaded(upload_date);
													continue 'main;
												}
											}
										}
										_ => {
											// println!();
											// dbg!(text);
											// println!();
											// panic!();
										}
									}
								}
							}
						}
					}
					// dbg!(node.value());
				}
			}
			#[cfg(feature = "a")]
			{
				while let Some(child) = sidestats_iter.next() {
					let node = extract_node_from_edge!(child);

					let value = node.value();
					dbg!(value);
				}
			}

			dbg!(&builder);
			builder.build().map_err(|e| anyhow::anyhow!(e))
		})
		.map(|item| item.unwrap())
		.collect::<Vec<_>>())
}

#[derive(Serialize, Deserialize, Builder, Debug)]
#[builder(derive(Debug))]
// #[serde(rename...)]
pub struct NewGroundsAudioMetadata {
	listens: u64,
	//CONSIDER: logging the users who favorited as well (endpoint example https://www.newgrounds.com/favorites/content/who/429941/3 (from dd a href id="faves_load"))
	faves: u64,
	downloads: u64,
	votes: Option<u64>,
	score: ScoreType,
	uploaded: String,
	// genre: String,
	// tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ScoreType {
	Score(f64),
	Waiting(String),
}
#[cfg(test)]
mod tests {
	// Note this useful idiom: importing names from outer (for mod tests) scope.
	// use super::*;

	#[test]
	fn test_download_files() {}
}
