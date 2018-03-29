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
extern crate hound;
extern crate cpal;


use clap::{App, Arg};
use restson::{RestClient,RestPath,Error};

use std::env;
use std::io;
use std::io::prelude::*;
use std::io::BufReader;
use std::io::Write;

#[derive(Serialize,Deserialize)]
struct InputConfig {
    text: Option<String>,
    ssml: Option<String>,
}

#[derive(Serialize,Deserialize)]
struct VoiceConfig {
    #[serde(rename = "languageCode")]
    language_code: String,

    #[serde(rename = "name")]
    name: String,

    #[serde(rename = "ssmlGender")]
    gender: String,
}

#[derive(Serialize,Deserialize)]
struct AudioConfig {
    #[serde(rename = "audioEncoding")]
    audio_encoding: String,

    #[serde(rename = "pitch")]
    pitch: f32,

    #[serde(rename = "speakingRate")]
    speaking_rate: f32,

    #[serde(rename = "volumeGainDb")]
    gain: f32,
}

#[derive(Serialize,Deserialize)]
struct SynthesizeRequest {
    #[serde(rename = "input")]
    input: InputConfig,

    #[serde(rename = "voice")]
    voice: VoiceConfig,

    #[serde(rename = "audioConfig")]
    audio_config: AudioConfig,
}

#[derive(Deserialize)]
struct SynthesizeResponse {
    #[serde(rename = "audioContent")]
    audio_content: String,
}

impl RestPath<String> for SynthesizeRequest {
    fn get_path(param: String) -> Result<String, Error>
    {
        Ok(format!("v1beta1/{}", param))
    }
}

fn sample_format(format: cpal::SampleFormat) -> hound::SampleFormat {
    match format {
        cpal::SampleFormat::U16 => hound::SampleFormat::Int,
        cpal::SampleFormat::I16 => hound::SampleFormat::Int,
        cpal::SampleFormat::F32 => hound::SampleFormat::Float,
    }
}

fn wav_spec_from_format(format: &cpal::Format) -> hound::WavSpec {
    hound::WavSpec {
        channels: format.channels as _,
        sample_rate: format.sample_rate.0 as _,
        bits_per_sample: (format.data_type.sample_size() * 8) as _,
        sample_format: sample_format(format.data_type),
    }
}

fn pause(message: &str) {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();

    write!(stdout, "{}", message).unwrap();
    stdout.flush().unwrap();

    let _ = stdin.read(&mut [0u8]).unwrap();
}

fn main() {

    let matches = App::new("Cloud Text-to-Speech")
                        .version("0.1.0")
                        .author("Graham Wihlidal <graham@wihlidal.ca>")
                        .about("Google Cloud text-to-speech prototype")
                        .arg(Arg::with_name("pitch")
                            .long("pitch")
                            .help("Optional speaking pitch, in the range [-20.0, 20.0]. 20 means increase 20 semitones from the original pitch. -20 means decrease 20 semitones from the original pitch.")
                            .takes_value(true))
                        .arg(Arg::with_name("rate")
                            .long("rate")
                            .help("Optional speaking rate/speed, in the range [0.25, 4.0]. 1.0 is the normal native speed supported by the specific voice. 2.0 is twice as fast, and 0.5 is half as fast. If unset(0.0), defaults to the native 1.0 speed. Any other values < 0.25 or > 4.0 will return an error.")
                            .takes_value(true))
                        .arg(Arg::with_name("gain")
                            .long("gain")
                            .help("Optional volume gain (in dB) of the normal native volume supported by the specific voice, in the range [-96.0, 16.0]. If unset, or set to a value of 0.0 (dB), will play at normal native signal amplitude. A value of -6.0 (dB) will play at approximately half the amplitude of the normal native signal amplitude. A value of +6.0 (dB) will play at approximately twice the amplitude of the normal native signal amplitude. Strongly recommend not to exceed +10 (dB) as there's usually no effective increase in loudness for any value greater than that.")
                            .takes_value(true))
                        .arg(Arg::with_name("key")
                            .help("Sets cloud API key")
                            .required(true)
                            .index(1))
                        .arg(Arg::with_name("input")
                            .help("Sets the input to synthesize (raw text or ssml)")
                            .required(true)
                            .index(2))
                        .arg(Arg::with_name("play")
                            .long("play")
                            .help("Enable synthesized audio playback"))
                        .arg(Arg::with_name("record")
                            .long("record")
                            .help("Enable synthesized audio recording"))
                        .arg(Arg::with_name("gender")
                            .long("gender")
                            .help("Optional preferred voice gender (i.e. MALE, FEMALE, NEUTRAL). If not set, the service will choose a voice based on the other parameters such as language code and voice name. Note that this is only a preference, not a requirement; if a voice of the appropriate gender is not available, the synthesizer should substitute a voice with a different gender rather than failing the request.")
                            .takes_value(true))
                        .arg(Arg::with_name("language")
                            .long("language")
                            .help("Optional voice language (i.e. en-US). The language (and optionally also the region) of the voice expressed as a BCP-47 language tag, e.g. en-US. This should not include a script tag (e.g. use 'cmn-cn' rather than 'cmn-Hant-cn'), because the script will be inferred from the input provided in the synthesis input. The TTS service will use this parameter to help choose an appropriate voice. Note that the TTS service may choose a voice with a slightly different language code than the one selected; it may substitute a different region (e.g. using en-US rather than en-CA if there isn't a Canadian voice available), or even a different language, e.g. using 'nb' (Norwegian Bokmal) instead of 'no' (Norwegian)")
                            .takes_value(true))
                        .arg(Arg::with_name("name")
                            .long("name")
                            .help("Optional voice name (i.e. en-US-Wavenet-D). If not set, the service will choose a voice based on the other parameters such as language code and voice gender.")
                            .takes_value(true))
                        .get_matches();

    let api_key = matches.value_of("key").unwrap();

    let synthesize_input = matches.value_of("input").unwrap();
    println!("Synthesizing input text: {}", synthesize_input);

    let pitch = value_t!(matches, "pitch", f32).unwrap_or(0.00);
    let gain = value_t!(matches, "gain", f32).unwrap_or(0.00);
    let speaking_rate = value_t!(matches, "rate", f32).unwrap_or(1.00);
    let gender = matches.value_of("gender").unwrap_or("MALE");
    let language = matches.value_of("language").unwrap_or("en-US");
    let voice_name = matches.value_of("name").unwrap_or("en-US-Wavenet-D");

    let mut client = RestClient::new("https://texttospeech.googleapis.com").unwrap();

    // https://cloud.google.com/storage/docs/json_api/v1/how-tos/authorizing
    let params = vec![("key", api_key)];

    let data = SynthesizeRequest {
        input: InputConfig {
            ssml: Some(String::from(synthesize_input)),
            text: None,
        },
        voice: VoiceConfig {
            language_code: String::from(language), // https://cloud.google.com/speech/docs/languages
            name: String::from(voice_name), // en-US-Wavenet-C (female)
            gender: String::from(gender), // MALE, FEMALE, NEUTRAL
        },
        audio_config: AudioConfig {
            audio_encoding: String::from("LINEAR16"), // OGG_OPUS, LINEAR16, MP3
            pitch: pitch,
            gain: gain,
            speaking_rate: speaking_rate,
        },
    };

    // https://cloudplatform.googleblog.com/2018/03/introducing-Cloud-Text-to-Speech-powered-by-Deepmind-WaveNet-technology.html
    // https://developers.google.com/web/updates/2014/01/Web-apps-that-talk-Introduction-to-the-Speech-Synthesis-API
    // https://cloud.google.com/speech/reference/rpc/google.cloud.speech.v1beta1
    // https://cloud.google.com/text-to-speech/docs/reference/rest/v1beta1/text/synthesize
    let resp: Result<SynthesizeResponse, Error> = client.post_capture_with(String::from("text:synthesize"), &data, &params);
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
            //let path = tmpfile.path().to_path_buf();
            tmpfile.write(bytes).unwrap();

            let play_path = env::temp_dir().join("speech-test.wav");
            tmpfile.persist(&play_path).unwrap();
            println!("Persisted response data to: {:?}", play_path);

            let record_path = env::temp_dir().join("record-test.wav");

            if matches.is_present("play") {
                println!("Playing synthesized audio");

                let endpoint = rodio::default_endpoint().unwrap();
                let mut sink = rodio::Sink::new(&endpoint);
                let play_file = std::fs::File::open(&play_path).unwrap();
                sink.append(rodio::Decoder::new(BufReader::new(play_file)).unwrap());
                sink.set_volume(1.0);
                sink.sleep_until_end();
            }

            if matches.is_present("record") {
                println!("Recording synthesized audio");

                // Setup the default input device and stream with the default input format.
                let device = cpal::default_input_device().expect("Failed to get default input device");
                println!("Default input device: {}", device.name());
                let format = device.default_input_format().expect("Failed to get default input format");
                println!("Default input format: {:?}", format);

                let event_loop = cpal::EventLoop::new();
                let stream_id = event_loop.build_input_stream(&device, &format)
                    .expect("Failed to build input stream");
                event_loop.play_stream(stream_id);

                let spec = wav_spec_from_format(&format);
                let writer = hound::WavWriter::create(&record_path, spec).unwrap();
                let writer = std::sync::Arc::new(std::sync::Mutex::new(Some(writer)));

                pause("Press enter to start recording...");
                let recording = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));

                // Run the input stream on a separate thread.
                let writer_2 = writer.clone();
                let recording_2 = recording.clone();
                std::thread::spawn(move || {
                    event_loop.run(move |_, data| {
                        // If we're done recording, return early.
                        if !recording_2.load(std::sync::atomic::Ordering::Relaxed) {
                            return;
                        }
                        // Otherwise write to the wav writer.
                        match data {
                            cpal::StreamData::Input { buffer: cpal::UnknownTypeInputBuffer::U16(buffer) } => {
                                if let Ok(mut guard) = writer_2.try_lock() {
                                    if let Some(writer) = guard.as_mut() {
                                        for sample in buffer.iter() {
                                            let sample = cpal::Sample::to_i16(sample);
                                            writer.write_sample(sample).ok();
                                        }
                                    }
                                }
                            },
                            cpal::StreamData::Input { buffer: cpal::UnknownTypeInputBuffer::I16(buffer) } => {
                                if let Ok(mut guard) = writer_2.try_lock() {
                                    if let Some(writer) = guard.as_mut() {
                                        for &sample in buffer.iter() {
                                            writer.write_sample(sample).ok();
                                        }
                                    }
                                }
                            },
                            cpal::StreamData::Input { buffer: cpal::UnknownTypeInputBuffer::F32(buffer) } => {
                                if let Ok(mut guard) = writer_2.try_lock() {
                                    if let Some(writer) = guard.as_mut() {
                                        for &sample in buffer.iter() {
                                            writer.write_sample(sample).ok();
                                        }
                                    }
                                }
                            },
                            _ => (),
                        }
                    });
                });

                pause("Press enter to finish recording...");
                recording.store(false, std::sync::atomic::Ordering::Relaxed);
                writer.lock().unwrap().take().unwrap().finalize().unwrap();
                println!("Recording {:?} complete!", &record_path);


            }
        }
    }
}
