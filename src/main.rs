use std::error::Error;

use parrot::client::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();

    let mut parrot = Client::default().await?;
    if let Err(why) = parrot.start().await {
        println!("Fatality! Parrot crashed because: {:?}", why);
    };

    Ok(())
}
