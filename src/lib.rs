use md5::{Digest, Md5};
use serde_derive::{Deserialize, Serialize};

pub struct Broker {
    pub id: String,
    pub host: String,
    pub port: u16,
}

#[derive(Deserialize)]
pub struct Request<'a> {
    data: &'a [u8],
    leading_zeros: u8,
}

#[derive(Serialize)]
pub struct Response<'a> {
    data: &'a [u8],
    leading_zeros: u8,
    nonce: u16,
}

fn digest_leading_zeroes(digest_bytes: &[u8]) -> u8 {
    let mut leading_zeros = 0;

    for byte in digest_bytes {
        let byte_leading_zeros = byte.leading_zeros();

        leading_zeros += byte_leading_zeros;

        if byte_leading_zeros != 8 {
            return leading_zeros as u8;
        }
    }

    leading_zeros as u8
}

pub fn handle_request<'a>(request: &'a Request) -> Response<'a> {
    let mut hasher = Md5::new();
    let mut nonce: u16 = 0;

    loop {
        hasher.update(request.data);
        hasher.update(nonce.to_ne_bytes());

        let digest = hasher.finalize_reset();

        if digest_leading_zeroes(digest.as_slice()) >= request.leading_zeros {
            let Request {
                data,
                leading_zeros,
            } = request;

            return Response {
                data,
                leading_zeros: *leading_zeros,
                nonce,
            };
        }
        nonce += 1;
    }
}

pub fn deserialize_payload<'a>(payload: &[u8]) -> Vec<u8> {
    serde_json::to_vec(&handle_request(&serde_json::from_slice(&payload).unwrap())).unwrap()
}
