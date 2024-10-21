// use std::io::Read;

// use axum::http::HeaderValue;
// use flate2::bufread::{DeflateDecoder, GzDecoder};
// use hyper::Response;

// use crate::{http_proxy::{create_simple_response_from_bytes, EpicResponse}, CustomError};

// // ====================================================================================================
// // This is just a poc/draft implementation for experimenting with the idea of a github custom server ..
// // If this is something that is worth pursuing, then it should be implemented as a proper feature with all the bells and whistles
// // not just a quick hack like this.
// // ====================================================================================================
// pub async fn handle(target:GitHubServer,req:hyper::Request<hyper::body::Incoming>) -> Result<EpicResponse,CustomError>{

//     let mut file_path = req.uri().path().trim_start_matches('/').to_string();

//     if file_path == "" {
        
//         let url = format!(
//             "https://api.github.com/repos/{owner}/{repo}/contents",
//             owner = target.owner,
//             repo = target.repo
//         );
//         let client = reqwest::Client::new();
//         let response = client.get(&url)
//             .header("user-agent", req.headers().get("user-agent").unwrap_or(&HeaderValue::from_static("odd-box")))
//             .send()
//             .await.map_err(|e|CustomError(format!("Failed to send request: {}", e).into()))?;
      
//         let decompressed_data = decompress_body(response).await?;   
//         let json : serde_json::value::Value = serde_json::de::from_str(std::str::from_utf8(&decompressed_data)
//             .map_err(|e|CustomError(format!("{e:?}")))?).map_err(|e|CustomError(format!("{e:?}")))?;

//         let jarr = json.as_array().ok_or(CustomError(format!("Expected array, got: {json:?}")))?;
//         let mut readme_file = None;
//         for item in jarr {
//             let item = item.as_object().ok_or(CustomError("Expected object".to_string()))?;
//             let name = item.get("name").ok_or(CustomError("Expected name".to_string()))?;
//             let l_name = name.as_str().ok_or(CustomError("failed to read name".into()))?.to_ascii_lowercase();
//             if l_name == "readme.md" || l_name == "read_me.md" || l_name == "read-me.md" || l_name == "read me.md"  {
//                 readme_file = Some(item);
//                 break;
//             }
//         }

//         if readme_file.is_none() {
//             for item in jarr {
//                 let item = item.as_object().ok_or(CustomError("Expected object".to_string()))?;
//                 let name = item.get("name").ok_or(CustomError("Expected name".to_string()))?;
//                 if name.as_str().ok_or(CustomError("invalid name".into()))?.to_ascii_lowercase() == "index.html" {
//                     readme_file = Some(item);
//                     break;
//                 }
//             }
//         }

//         if let Some(readme_file) = readme_file {
//             let download_url = readme_file.get("download_url").ok_or(CustomError("Expected download_url".to_string()))?;
//             let download_url = download_url.as_str().ok_or(CustomError("Expected string".to_string()))?;
//             file_path = download_url.split('/').last().ok_or(CustomError("bad path".into()))?.to_string();
//         }

//     }
//     if file_path.ends_with('/') {
//         let url = format!(
//             "https://api.github.com/repos/{owner}/{repo}/contents/{file_path}",
//             owner = target.owner,
//             repo = target.repo,
//             file_path = file_path
//         );
//         let client = reqwest::Client::new();
//         let response = client.get(&url)
//             .header("user-agent", req.headers().get("user-agent").unwrap_or(&HeaderValue::from_static("odd-box")))
//             .send()
//             .await.map_err(|e|CustomError(format!("Failed to send request: {}", e).into()))?;
      
//         let decompressed_data = decompress_body(response).await?;   
//         let json : serde_json::value::Value = serde_json::de::from_str(std::str::from_utf8(&decompressed_data).map_err(|e|CustomError(format!("{e:?}")))?).map_err(|e|CustomError(format!("{e:?}")))?;
//         let jarr = json.as_array().ok_or(CustomError("Expected array".to_string()))?;
        
//         let mut html = String::new();
//         html.push_str(&format!("<a href=\"../\">../</a></br>"));
//         for item in jarr {
//             let item_name = item.get("name")
//                 .ok_or(CustomError("Expected name".to_string()))?.as_str().ok_or(CustomError("invalid item name".into()))?;

//             let item_path = item.get("path").ok_or(CustomError("Expected path".to_string()))?.as_str().ok_or(CustomError("invalid item path".into()))?;
//             let is_dir = item.get("type").ok_or(CustomError("Expected type".to_string()))?.as_str().ok_or(CustomError("invalid item type".into()))? == "dir";
//             if is_dir {
//                 html.push_str(&format!("<a title=\"dir: {item_path}\" href=\"{}/\">{}</a></br>\n", item_name,item_name));
//             } else {
//                 html.push_str(&format!("<a title=\"file: {item_path}\"href=\"{}\">{}</a></br>\n", item_name,item_name));
//             }
//         }
//         let rr : hyper::Response<bytes::Bytes> = Response::builder()
//             .status(200)
//             .header("content-type", "text/html")
//             .body(html.into())
//             .map_err(|e|CustomError(format!("Failed to create response: {}", e).into()))?;
//         return create_simple_response_from_bytes(rr).await
//     }



//     let repo = target.repo;
//     let owner = target.owner;
//     let branch = target.branch.as_deref().unwrap_or("main");

//     let url = 
//         if file_path.starts_with("actions") {            
//             format!(
//                 "https://github.com/{owner}/{repo}/{file_path}"
//             )
//         } else {
//             format!(
//                 "https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{file_path}"
//             )
//         };
        

//     let client = reqwest::Client::new();
    
//     let mut request_builder = client .get(&url);

//     for h in req.headers() {
//         if h.0 == "host" {
//             continue;
//         }
//         request_builder = request_builder.header(h.0,h.1);
//     }

//     request_builder = request_builder.header("referer","https://github.com");


//     let response = request_builder
//         .send()
//         .await.map_err(|e|CustomError(format!("Failed to send request: {}", e).into()))?;

//     let response_status = response.status();

//     let mut headers = response.headers().clone();
    

//     let response_bytes : bytes::Bytes;

//     let u = file_path.to_ascii_lowercase();
    
//     if (u.ends_with(".html") || u.ends_with(".md")) && response_status == 200 {

//         if u.ends_with(".md") {

//             let decompressed_data = decompress_body(response).await?;

            
//             let text = String::from_utf8(decompressed_data).map_err(|e|CustomError(format!("{e:?}")))?
//                 .replace(&format!("https://github.com/{owner}/{repo}/blob/{branch}/"), "/")
//                 .replace(&format!("https://github.com/{owner}/{repo}/"), "/");
//                         //let md = markdown::to_html(&text);
//             let mut mo = markdown::Options::default();
//             mo.parse.constructs.gfm_table = true;
//             let html = markdown::to_html_with_options(&text, &mo).map_err(|e|CustomError(format!("{e:?}")))?;

//             let html = format!("
//                 <!DOCTYPE html>
//                 <html>
//                 <head>
//                 <link rel='stylesheet' href='https://cdn.jsdelivr.net/npm/github-markdown-css'>
//                 <style>
//                 body {{
//                     background-color: #0d1117; /* Dark background */
//                     color: #c9d1d9; /* Light text for readability */
//                     padding: 3rem;
//                 }}
//                 a {{
//                     color: #58a6ff; /* Link color matching GitHub's dark theme */
//                     text-decoration: none;
//                 }}
//                 a:hover {{
//                     text-decoration: underline; /* Underline on hover for better UX */
//                 }}
//                 pre, code {{
//                     background-color: #161b22; /* Dark background for code blocks */
//                     color: #e1e4e8; /* Light text for code */
//                     border-radius: 6px;
//                     padding: 0.2em 0.4em;
//                 }}
//                 pre {{
//                     padding: 1em;
//                     overflow: auto;
//                 }}
//                 table {{
//                     border: 1px solid #30363d; /* Table borders */
//                     width: 100%;
//                 }}
//                 th, td {{
//                     padding: 8px 12px;
//                     border: 1px solid #30363d; /* Cell borders */
//                 }}
//                 blockquote {{
//                     border-left: 4px solid #58a6ff; /* Blockquote styling */
//                     color: #8b949e;
//                     padding: 1em;
//                 }}
//                 img {{
//                     max-width: 100%; /* Responsive images */
//                     height: auto;
//                     display: block;
//                     margin: 1em 0;
//                 }}
//                 </style>
//                 </head>
//                 <body class='markdown-body'>
//                 {html}
//                 </body>
//                 </html>
//             ");
//             headers.insert("content-type", "text/html".parse().map_err(|e|CustomError(format!("{e:?}")))?);
//             headers.remove("content-length");
//             headers.remove("content-encoding");
//             response_bytes = html.bytes().collect();
//         } else {
//             headers.insert("content-type", "text/html".parse().map_err(|e|CustomError(format!("{e:?}")))?);
//             response_bytes = response.bytes().await.map_err(|e|CustomError(format!("{e:?}")))?;
//         }
//         // add or replace content-type header:
        

//     } else {
//         response_bytes = response.bytes().await.map_err(|e|CustomError(format!("{e:?}")))?;
//     }

//     headers.remove("content-security-policy");
    

//     let mut rr = Response::builder()
//         .status(response_status)
//         .body(response_bytes)
//         .map_err(|e|CustomError(format!("Failed to create response: {}", e).into()))?;

//     for (k,v) in headers {
//         if let Some(k) = k {
//             rr.headers_mut().insert(k,v);
//         } else {
//             tracing::error!("Failed to insert header: {:?} -> {:?}", k,v);
//         }
//     }



//     create_simple_response_from_bytes(rr).await

// }


// async fn decompress_body(response:reqwest::Response) -> Result<Vec<u8>, CustomError> {
    
//     let content_encoding = response.headers().get(reqwest::header::CONTENT_ENCODING);

//     let mut decompressed_data = Vec::new();


//     match content_encoding.and_then(|v| v.to_str().ok()) {
//         Some("gzip") => {
//             let body = response.bytes().await.map_err(|e|CustomError(format!("{e:?}")))?;
//             let mut decoder = GzDecoder::new(body.as_ref());
//             decoder.read_to_end(&mut decompressed_data).map_err(|e|CustomError(format!("{e:?}")))?;
//         },
//         Some("deflate") => {
//             let body = response.text().await.map_err(|e|CustomError(format!("{e:?}")))?;
//             let mut decoder = DeflateDecoder::new(body.as_bytes());
//             decoder.read_to_end(&mut decompressed_data).map_err(|e|CustomError(format!("{e:?}")))?;
//         },
//         _ => {
//             decompressed_data = response.bytes().await.map_err(|e|CustomError(format!("{e:?}")))?.to_vec();
//         }
//     }

//     Ok(decompressed_data)
// }