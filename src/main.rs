#![feature(type_name_of_val)]
use backoff::{retry, ExponentialBackoff};
use derive_builder::Builder;
use ego_tree::iter::Edge;
use regex::Regex;
use reqwest::{redirect::Policy, StatusCode, Url};
use scraper::{node::Node, Html, Selector};
use serde::Serialize;
use std::{
	io::Write,
	path::{Path, PathBuf},
	time::Duration,
};

fn main() -> anyhow::Result<()> {
	println!("Start of newgrounds-audio-downloader");
	//TODO: get from CLI
	get_files("keosni391")?;
	Ok(())
}

pub fn get_files(username: &str) -> anyhow::Result<()> {
	//TODO: get this stuff and more from the args (but keep dl_type as audio as the default and probably only supported download type (more would require name change and archive with notice of new crate if published))
	let dl_type = "audio";
	let dl_directory = "/mnt/ehdd/DH/music/artists/K-391/newgrounds";

	let template = format!("https://{username}.newgrounds.com/{dl_type}/page/");
	let mut all_matches = vec![];

	//TODO: possibly scrape the songs' listen pages instead (probably more robust, and likely additional information can be retrieved from there) (TODO: check if likes, downloads, upload date, etc are already archived by us)

	let mut pages_i = 1;
	let matches_re = Regex::new(
		format!(
			"<a href=.*?newgrounds\\.com.*?{dl_type}.*?listen.*?([0-9]+)\" class=\"item-audiosubmission \">"
		)
		.as_str(),
	)?;

	let client = reqwest::blocking::ClientBuilder::new()
		.redirect(Policy::custom(|attempt| {
			//expected_example: "https://keosni391.newgrounds.com/audio/page/3/" => "https://keosni391.newgrounds.com/audio/page/3/?page=2", where the artist keosni391 has exactly 2 pages of songs
			let original_url = attempt.previous()[0].clone();
			if attempt.previous().len() != 1 {
				panic!("multiple previous URLs!");
			}
			let redirected_url = attempt.url().clone();
			println!("redirection of {} to {}", original_url, redirected_url);

			// This code is super messy and bad and could be refactored to be much better
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
		let url = Url::parse(format!("{template}{pages_i}").as_str())?;
		println!("fetching {url}");
		let resp = client.get(url).send();
		let content = match resp {
			//CHECK: I think utf-8 is the default used by the library so I could just do `resp.text()` IIRC, although I'm not even sure the response's encoding is UTF-8 in the first place, so this might all be bad anyways
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
		pages_i += 1;
	}
	println!("{} songs found ({pages_i} pages)", all_matches.len());

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

	let files = song_ids
		.iter()
		.map(|id| Path::new(dl_directory).join(id))
		.collect::<Vec<_>>();

	let urls = song_ids
		.iter()
		.map(|id| Url::parse(&format!("http://www.newgrounds.com/audio/download/{id}")).unwrap())
		.collect();

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
//TODO: make async
pub fn download_files(urls: Vec<Url>, mut files: Vec<PathBuf>) -> anyhow::Result<()> {
	assert!(urls.len() == files.len());
	//TODO: remove file field from program - new impl dependent on whether it's guaranteed that file names will always be present from the response and never conflict causing overwrites with different ones
	urls.into_iter()
		.zip(files.iter_mut())
		.for_each(|(url, file)| {
			let backoff = ExponentialBackoff::default();
			let instant = std::time::Instant::now();
			let op = || {
				//TODO: overwrite the previous text instead of spamming the terminal with new progress messages
				println!(
					"backoff due to rate-limiting for url {url}: {:?} seconds",
					instant.elapsed().as_secs()
				);
				//TODO: [re-]use a client instead
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
									//CHECK: I'm not sure it's even possible for httpdate to parse this stuff. This code may be pure bloat
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
	Ok(())
}

pub fn get_metadata_for_ids(ids: &[&str]) -> anyhow::Result<Vec<NewGroundsAudioMetadata>> {
	// both variants contain the same stuff which we want to handle the same way (I think, at least), and I do the same thing everywhere so this simple macro is helpful
	macro_rules! extract_value_from_edge {
		($edge:ident) => {
			match $edge {
				Edge::Open(node) => node,
				Edge::Close(node) => node,
			}
			.value()
		};
	}
	let base_metadata_url = Url::parse("https://www.newgrounds.com/audio/listen/")?;
	Ok(ids
		.iter()
		.map(|id| -> anyhow::Result<NewGroundsAudioMetadata> {
			let url = base_metadata_url.clone().join(id)?;
			println!("{}", url);
			//TODO: (re-)use a client instead
			let response = reqwest::blocking::get(url)?;
			let html = Html::parse_document(&response.text()?);
			let mut builder = NewGroundsAudioMetadataBuilder::default();
			let selector = Selector::parse("#sidestats").unwrap();
			let mut stats_selector = html.select(&selector);
			let sidestats_div = stats_selector.next().unwrap();
			assert!(stats_selector.next() == None);
			let whitespace_re = Regex::new(r"^\s+$").unwrap();
			let pending_score_re = Regex::new(r"^\s*Waiting for \d+ more votes?\s*$").unwrap();
			let mut sidestats_iter = sidestats_div.traverse();
			#[cfg(not(feature = "just_print_all_nodes"))]
			{
				'main: while let Some(child) = sidestats_iter.next() {
					let value = extract_value_from_edge!(child);
					if let Node::Element(element) = value {
						if element.name() == "dt" {
							if let Some(child) = sidestats_iter.next() {
								let value = extract_value_from_edge!(child);
								if let Node::Text(text) = value {
									#[allow(clippy::while_let_on_iterator)]
									match &**text {
										"Listens" => {
											sidestats_iter.next(); // Listens clone (ending/closing part - applies to below too)
											sidestats_iter.next(); // dt clone
											sidestats_iter.next(); // whitespace
											sidestats_iter.next(); // whitespace clone
											if let Some(child) = sidestats_iter.next() {
												let value = extract_value_from_edge!(child);
												if let Node::Element(element) = value {
													if element.name() == "dd" {
														if let Some(child) = sidestats_iter.next() {
															let value =
																extract_value_from_edge!(child);
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
												let value = extract_value_from_edge!(child);
												//CHECK: can a macro be used for the other block of exactly duplicated and nearly exactly duplicated lines here?
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
												let value = extract_value_from_edge!(child);
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
												let value = extract_value_from_edge!(child);
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
												let value = extract_value_from_edge!(child);
												if let Node::Text(text) = value {
													let text = (**text).replace(',', "");
													if let Ok(score) = text.parse::<f64>() {
														builder.score(ScoreType::Score(score));
														continue 'main;
													} else if pending_score_re.is_match(&text) {
														let text = text.trim().to_owned();
														builder.score(ScoreType::Waiting(text));
														builder.votes(None);
														continue 'main;
													}
												}
											}
										}
										"Uploaded" => {
											//WATCH: these calls might be able to skip over some information we want. Should change this to be more robust. This goes for the other calls in the upper match as well. Should check and handle them just in case
											sidestats_iter.next();
											sidestats_iter.next();
											sidestats_iter.next();
											let mut upload_date = String::new();
											// the date comes in two separate Texts, with no whitespace separating one Text from the other
											let mut date_parts_found: u8 = 0;
											while let Some(child) = sidestats_iter.next() {
												sidestats_iter.next();
												let value = extract_value_from_edge!(child);
												if let Node::Text(text) = value {
													let text = &(**text);
													if text.is_empty() {
														continue;
													}
													if whitespace_re.is_match(text) {
														continue;
													}
													if date_parts_found == 1 {
														upload_date.push(' ');
													}
													upload_date.push_str(text);
													date_parts_found += 1;
												} else if date_parts_found > 1 {
													println!("upload_date: {}", &upload_date);
													builder.uploaded(upload_date);
													continue 'main;
												}
											}
										}
										_ => {}
									}
								}
							}
						}
					}
				}
			}
			// this feature and code was used sometimes in developing. Could probably just remove it all but no real reason to do so just yet
			#[cfg(feature = "just_print_all_nodes")]
			{
				while let Some(child) = sidestats_iter.next() {
					let value = extract_value_from_edge!(child);
					dbg!(value);
				}
			}

			dbg!(&builder);
			builder.build().map_err(|e| anyhow::anyhow!(e))
		})
		.map(|item| item.unwrap())
		.collect::<Vec<_>>())
}

#[derive(Serialize, Builder, Debug)]
#[builder(derive(Debug))]
pub struct NewGroundsAudioMetadata {
	listens: u64,
	//CONSIDER: logging the users who favorited as well (endpoint example https://www.newgrounds.com/favorites/content/who/429941/3 (from <dd ...<a href .. id="faves_load"))
	faves: u64,
	downloads: u64,
	// if there aren't enough votes, both the score and the votes aren't normal, with one not being displayed and the other displaying a message about it waiting for X votes
	votes: Option<u64>,
	score: ScoreType,
	uploaded: String,
	//TODO: the genre and tags
	// genre: String,
	// tags: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
pub enum ScoreType {
	// an f32 might be sufficient but who really cares about the increased cost of using an f64 just to possibly lose less info
	Score(f64),
	Waiting(String),
}
