use markdown::Options;
use rouille::{post_input, router, try_or_400};
use std::{
    fs, io,
    sync::{Arc, Mutex},
};

fn main() {
    dotenv::dotenv().ok();

    let client = Arc::new(Mutex::new(
        postgres::Client::connect(
            &std::env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            postgres::NoTls,
        )
        .expect("Could not establish PG connection"),
    ));

    let host = std::env::var("HOST").unwrap();
    let port = std::env::var("PORT").unwrap();

    println!("Now listening on {host}:{port}");
    rouille::start_server(format!("{host}:{port}"), move |request| {
        rouille::log(request, io::stdout(), || {
            let page = fs::read_to_string("index.html").unwrap();
            router!(request,
                (GET) ["/"] => {
                    rouille::Response::html(page)
                },

                (GET) ["/favicon.ico"] => {
                    let res = rouille::match_assets(request, "public");
                    if res.is_success() {
                        res
                    } else {
                        rouille::Response::empty_404()
                    }
                },

                (POST) ["/"] => {
                    let data = try_or_400!(post_input!(request, {
                        title: String,
                        descr: String,
                        category: String,
                        file: String,
                    }));

                    let file = markdown::to_html_with_options(&data.file, &Options::gfm()).unwrap();

                    println!("Received data: {:?}", file);

                    client
                      .lock()
                      .unwrap()
                      .execute("INSERT INTO posts(title, descr, category, content) VALUES ($1, $2, $3, $4)",
                         &[&data.title, &data.descr, &data.category, &file])
                      .unwrap();

                    rouille::Response::html("Success! <a href=\"/\">Go back</a>.")
                },

                _ => rouille::Response::empty_404()
            )
        })
    });
}
