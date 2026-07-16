#![allow(dead_code, unused_variables, unused_imports)]

use tokio::time::{sleep, Duration};



#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
   worker::run().await 
}


/*   note 

onchain call positions sur les 6 positions les plus a risque

*/


