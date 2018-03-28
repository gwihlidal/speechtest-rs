#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;
extern crate restson;
extern crate base64;
extern crate tempfile;
extern crate rodio;
#[macro_use]
extern crate clap;

use clap::{App, Arg};
use restson::{RestClient,RestPath,Error};

use std::env;
use std::io::BufReader;
use std::io::Write;

#[derive(Serialize,Deserialize)]
struct InputConfig {
    text: String,
}

#[derive(Serialize,Deserialize)]
struct VoiceConfig {
    #[serde(rename = "languageCode")]
    language_code: String,

    name: String,
}

#[derive(Serialize,Deserialize)]
struct AudioConfig {
    #[serde(rename = "audioEncoding")]
    audio_encoding: String,

    pitch: f32,

    #[serde(rename = "speakingRate")]
    speaking_rate: f32,
}

#[derive(Serialize,Deserialize)]
struct SynthesizeRequest {
    input: InputConfig,
    voice: VoiceConfig,
    #[serde(rename = "audioConfig")]
    audio_config: AudioConfig,
}

#[derive(Deserialize)]
struct HttpBinPostResp {
    #[serde(rename = "audioContent")]
    audio_content: String,
}

impl RestPath<String> for SynthesizeRequest {
    fn get_path(param: String) -> Result<String, Error>
    {
        Ok(format!("v1beta1/{}", param))
    }
}

fn main() {

    let matches = App::new("Cloud Text-to-Speech")
                        .version("0.0.1")
                        .author("Graham Wihlidal <graham@wihlidal.ca>")
                        .about("Google Cloud text-to-speech prototype")
                        .arg(Arg::with_name("pitch")
                            .long("pitch")
                            .help("Sets the synthesize pitch")
                            .takes_value(true))
                        .arg(Arg::with_name("rate")
                            .long("rate")
                            .help("Sets the synthesize speaking rate")
                            .takes_value(true))
                        .arg(Arg::with_name("key")
                            .help("Sets cloud API key")
                            .required(true)
                            .index(1))
                        .arg(Arg::with_name("input")
                            .help("Sets the input text to synthesize")
                            .required(true)
                            .index(2))
                        .arg(Arg::with_name("play")
                            .long("play")
                            .help("Enable synthesized audio playback"))
                        .get_matches();

    let api_key = matches.value_of("key").unwrap();

    let synthesize_input = matches.value_of("input").unwrap();
    println!("Synthesizing input text: {}", synthesize_input);

    let pitch = value_t!(matches, "pitch", f32).unwrap_or(0.00);
    let speaking_rate = value_t!(matches, "rate", f32).unwrap_or(1.00);

    let mut client = RestClient::new("https://texttospeech.googleapis.com").unwrap();

    // https://cloud.google.com/storage/docs/json_api/v1/how-tos/authorizing
    let params = vec![("key", api_key)];

    let data = SynthesizeRequest {
        input: InputConfig {
            text: String::from(synthesize_input),
        },
        voice: VoiceConfig {
            language_code: String::from("en-US"), // https://cloud.google.com/speech/docs/languages
            name: String::from("en-US-Wavenet-D"),
        },
        audio_config: AudioConfig {
            audio_encoding: String::from("LINEAR16"), // OGG_OPUS, LINEAR16, MP3
            pitch: pitch,
            speaking_rate: speaking_rate,
        },
    };

    // https://cloudplatform.googleblog.com/2018/03/introducing-Cloud-Text-to-Speech-powered-by-Deepmind-WaveNet-technology.html
    // https://developers.google.com/web/updates/2014/01/Web-apps-that-talk-Introduction-to-the-Speech-Synthesis-API
    // https://cloud.google.com/speech/reference/rpc/google.cloud.speech.v1beta1
    // https://cloud.google.com/text-to-speech/docs/reference/rest/v1beta1/text/synthesize
    let resp: Result<HttpBinPostResp, Error> = client.post_capture_with(String::from("text:synthesize"), &data, &params);
    match resp {
        Err(err) => {
            println!("Failed processing request: {:?}", err);

            // Print out serialized request
            let serialized = serde_json::to_string(&data).unwrap();
            println!("Serialized request is: {}", serialized);
        },
        Ok(val) => {
            let bytes_vec = base64::decode(&val.audio_content).unwrap();
            let bytes = bytes_vec.as_slice();

            let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
            let path = tmpfile.path().to_path_buf();
            tmpfile.write(bytes).unwrap();

            let persist_path = env::temp_dir().join("speech-test.wav");
            tmpfile.persist(&persist_path).unwrap();
            println!("Persisted response data to: {:?}", path);

            if matches.is_present("play") {
                println!("Playing synthesized audio");
                let endpoint = rodio::default_endpoint().unwrap();
                let mut sink = rodio::Sink::new(&endpoint);
                let play_file = std::fs::File::open(persist_path).unwrap();
                sink.append(rodio::Decoder::new(BufReader::new(play_file)).unwrap());
                sink.set_volume(1.0);
                sink.sleep_until_end();
            }
        }
    }
}
