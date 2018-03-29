#[macro_use] extern crate serde_derive;
extern crate serde;
extern crate serde_json;
extern crate restson;
extern crate base64;
extern crate tempfile;
extern crate rodio;
#[macro_use] extern crate clap;
extern crate hound;
extern crate cpal;
extern crate audrey;
extern crate file;

use clap::{App, Arg, ArgMatches};
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

#[derive(Serialize,Deserialize)]
struct RecognitionConfig {
    #[serde(rename = "encoding")]
    encoding: String,

    #[serde(rename = "sampleRateHertz")]
    sample_rate_hz: f32,

    #[serde(rename = "languageCode")]
    language: String,

    #[serde(rename = "maxAlternatives")]
    max_alternatives: i32,

    #[serde(rename = "profanityFilter")]
    profanity_filter: bool,

    #[serde(rename = "speechContexts")]
    contexts: Vec<RecognitionSpeechContext>,

    #[serde(rename = "enableWordTimeOffsets")]
    enable_word_time_offsets: bool,
}

#[derive(Serialize,Deserialize)]
struct RecognitionSpeechContext {
    #[serde(rename = "phrases")]
    phrases: Vec<String>,
}

#[derive(Serialize,Deserialize)]
struct RecognitionAudio {
    #[serde(rename = "content")]
    content: String,
}

#[derive(Serialize,Deserialize)]
struct RecognizeRequest {
    #[serde(rename = "config")]
    config: RecognitionConfig,

    #[serde(rename = "audio")]
    audio: RecognitionAudio,
}

#[derive(Deserialize)]
struct SpeechRecognitionWordInfo {
    #[serde(rename = "startTime")]
    _start_time: String,

    #[serde(rename = "endTime")]
    _end_time: String,

    #[serde(rename = "word")]
    _word: String,
}

#[derive(Deserialize)]
struct SpeechRecognitionAlternative {
    #[serde(rename = "transcript")]
    transcript: String,

    #[serde(rename = "confidence")]
    confidence: f32,

    #[serde(default)]
    #[serde(rename = "words")]
    words: Vec<SpeechRecognitionWordInfo>,
}

#[derive(Deserialize)]
struct SpeechRecognitionResult {
    #[serde(default)]
    #[serde(rename = "alternatives")]
    alternatives: Vec<SpeechRecognitionAlternative>,
}

#[derive(Deserialize)]
struct RecognizeResponse {
    #[serde(default)]
    #[serde(rename = "results")]
    results: Vec<SpeechRecognitionResult>,
}

impl RestPath<String> for SynthesizeRequest {
    fn get_path(param: String) -> Result<String, Error>
    {
        Ok(format!("v1beta1/{}", param))
    }
}

impl RestPath<String> for RecognizeRequest {
    fn get_path(param: String) -> Result<String, Error>
    {
        Ok(format!("v1/{}", param))
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

fn enumerate_audio() {
    println!("Default Input Device:\n  {:?}", cpal::default_input_device().map(|e| e.name()));
    println!("Default Output Device:\n  {:?}", cpal::default_output_device().map(|e| e.name()));

    let devices = cpal::devices();
    println!("Devices: ");
    for (device_index, device) in devices.enumerate() {
        println!("{}. \"{}\"",
                 device_index + 1,
                 device.name());

        // Input formats
        if let Ok(fmt) = device.default_input_format() {
            println!("  Default input stream format:\n    {:?}", fmt);
        }
        let mut input_formats = match device.supported_input_formats() {
            Ok(f) => f.peekable(),
            Err(e) => {
                println!("Error: {:?}", e);
                continue;
            },
        };
        if input_formats.peek().is_some() {
            println!("  All supported input stream formats:");
            for (format_index, format) in input_formats.enumerate() {
                println!("    {}.{}. {:?}", device_index + 1, format_index + 1, format);
            }
        }

        // Output formats
        if let Ok(fmt) = device.default_output_format() {
            println!("  Default output stream format:\n    {:?}", fmt);
        }
        let mut output_formats = match device.supported_output_formats() {
            Ok(f) => f.peekable(),
            Err(e) => {
                println!("Error: {:?}", e);
                continue;
            },
        };
        if output_formats.peek().is_some() {
            println!("  All supported output stream formats:");
            for (format_index, format) in output_formats.enumerate() {
                println!("    {}.{}. {:?}", device_index + 1, format_index + 1, format);
            }
        }
    }
}

fn synthesize(args: &ArgMatches) {
    let api_key = args.value_of("key").unwrap();

    let synthesize_input = args.value_of("input").unwrap();
    println!("Synthesizing input text: {}", synthesize_input);

    let pitch = value_t!(args, "pitch", f32).unwrap_or(0.00);
    let gain = value_t!(args, "gain", f32).unwrap_or(0.00);
    let speaking_rate = value_t!(args, "rate", f32).unwrap_or(1.00);
    let gender = args.value_of("gender").unwrap_or("MALE");
    let language = args.value_of("language").unwrap_or("en-US");
    let voice_name = args.value_of("name").unwrap_or("en-US-Wavenet-D");

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

            if args.is_present("play") {
                println!("Playing synthesized audio");

                let endpoint = rodio::default_endpoint().unwrap();
                let mut sink = rodio::Sink::new(&endpoint);
                let play_file = std::fs::File::open(&play_path).unwrap();
                sink.append(rodio::Decoder::new(BufReader::new(play_file)).unwrap());
                sink.set_volume(1.0);
                sink.sleep_until_end();
            }
        }
    }
}

fn record_audio(record_path: &std::path::PathBuf) {
    // Setup the default input device and stream with the default input format.
    let device = cpal::default_input_device().expect("Failed to get default input device");
    let format = device.default_input_format().expect("Failed to get default input format");

    println!("Recording input format: {:?}", format);

    let event_loop = cpal::EventLoop::new();
    let stream_id = event_loop.build_input_stream(&device, &format)
        .expect("Failed to build input stream");
    event_loop.play_stream(stream_id);

    let spec = wav_spec_from_format(&format);
    //println!("spec - channels:{} sample_rate:{} bits_per_sample:{}", spec.channels, spec.sample_rate, spec.bits_per_sample);

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
    //std::thread::sleep(std::time::Duration::from_secs(3));
    recording.store(false, std::sync::atomic::Ordering::Relaxed);
    writer.lock().unwrap().take().unwrap().finalize().unwrap();
    println!("Recording {:?} complete!", &record_path);
}

fn convert_audio(record_path: &std::path::PathBuf) {

    let record_spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44_100,//spec.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut wav = audrey::open(&record_path).unwrap();
    let buffer = wav.frames::<[i16; 2]>()
        .map(Result::unwrap)
        .map(|f| f)
        .collect::<Vec<_>>();

    let mut writer = hound::WavWriter::create(&record_path, record_spec).unwrap();
    for sample in buffer.iter() {
        let sample_left  = sample[0] as f64;
        let sample_right = sample[1] as f64;
        writer.write_sample(((sample_left + sample_right) / 2.0) as i16).unwrap();
    }

    println!("{}", "Playback output format: Format { channels: 1, sample_rate: SampleRate(44100), data_type: I16 }");
}

fn recognize(args: &ArgMatches) {
    if args.is_present("record") {
        println!("Recording synthesized audio");

        let record_path = env::temp_dir().join("record-test.wav");

        record_audio(&record_path);
        convert_audio(&record_path);

        let api_key = args.value_of("key").unwrap();

        let mut client = RestClient::new("https://speech.googleapis.com").unwrap();

        // https://cloud.google.com/storage/docs/json_api/v1/how-tos/authorizing
        let params = vec![("key", api_key)];

        let encoding = "LINEAR16";
        let sample_rate_hz = 44_100.0;
        let language = "en-US";
        let max_alternatives = 0;
        let profanity_filter = false;
        let contexts: Vec<RecognitionSpeechContext> = vec![];
        let enable_word_time_offsets = false;

        let mut audio_file = std::fs::File::open(&record_path).unwrap();
        let mut audio_data = Vec::new();
        audio_file.read_to_end(&mut audio_data).unwrap();
        let audio_content: String = base64::encode(&audio_data);

        let data = RecognizeRequest {
            config: RecognitionConfig {
                encoding: String::from(encoding),
                sample_rate_hz: sample_rate_hz,
                language: String::from(language),
                max_alternatives: max_alternatives,
                profanity_filter: profanity_filter,
                contexts: contexts,
                enable_word_time_offsets: enable_word_time_offsets,
            },
            audio: RecognitionAudio {
                content: audio_content,
            },
        };

        let resp: Result<RecognizeResponse, Error> = client.post_capture_with(String::from("speech:recognize"), &data, &params);
        match resp {
            Err(err) => {
                println!("Failed processing request: {:?}", err);

                // Print out serialized request
                let serialized = serde_json::to_string(&data).unwrap();
                file::put("request_debug.txt", &serialized).expect("Failed to get write out serialized debug data");
            },
            Ok(val) => {
                println!("Length of results is {}", val.results.len());

                let mut transcript = String::new();
                for result in val.results {
                    if result.alternatives.len() > 0 {
                        let alternative_info = format!(" (Confidence: {})", &result.alternatives[0].confidence);
                        transcript += &result.alternatives[0].transcript;
                        transcript += &alternative_info;
                        if result.alternatives[0].words.len() > 0 {

                        }
                    }
                }

                println!("Recognition result: {}", transcript);
            }
        }
    }
}

fn main() {
    let matches = App::new("Cloud Speech Synthesis and Recognition")
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
                        .arg(Arg::with_name("enumerate")
                            .long("enumerate")
                            .help("Enable audio device enumeration"))
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

    if matches.is_present("enumerate") {
        enumerate_audio();
    }

    synthesize(&matches);
    recognize(&matches);
}
