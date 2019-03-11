extern crate gstreamer as gst;
use gst::prelude::*;
use std::{thread,time};

mod macos_workaround;
fn main() {
    macos_workaround::run(|| {
        gst::init().unwrap();
        let uri =
            "file:///Users/raytiley/Downloads/dn2019-0307-hd.mp4";
        let pipeline = gst::parse_launch(&format!("uridecodebin uri={} name=dn dn. ! x264enc speed-preset=superfast ! mux. dn. ! audioconvert ! audioresample ! audio/x-raw,channels=2,rate=48000 ! avenc_aac !  mux. mpegtsmux name=mux ! queue ! rtpmp2tpay ! udpsink host=127.0.0.1 port=6666", uri)).unwrap();

        pipeline
            .set_state(gst::State::Playing)
            .expect("Unable to set the pipeline to the `Playing` state");

        let five_secs = time::Duration::from_secs(5);
        thread::sleep(five_secs);

        let (audio_proxysink, video_proxysink, base_time) = start_recv_pipeline();

        //let five_secs = time::Duration::from_secs(5);
        //thread::sleep(five_secs);

        start_output_pipeline(audio_proxysink, video_proxysink, base_time);

        println!("Hit return to quit.");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
    });

}

fn make_element(factory_name: &str, element_name: &str, id: &str) -> gst::Element {
    let name = format!("vs-{}-{}", element_name, id);
    gst::ElementFactory::make(&factory_name, name.as_str()).unwrap()
}


fn start_recv_pipeline() -> (gst::Element, gst::Element, gst::ClockTime) {
    println!("Starting Receiving Pipeline...");
    let rtp_name = "test";
    let udpsrc = make_element("udpsrc", "rtp-udpsrc", &rtp_name);

    udpsrc.set_property_from_str("caps", "application/x-rtp,media=video,encoding-name=MP2T,clock-rate=90000");
    udpsrc.set_property_from_str("address", &"192.168.0.10");
    udpsrc.set_property_from_str("port", &"4444");

    let decodebin = make_element("decodebin", "rtp-decodebin", &rtp_name);
    let network_queue = make_element("queue", "network-queue", &rtp_name);
    let rtpjitterbuffer = make_element("rtpjitterbuffer", "jitterbuffer", &rtp_name);

    let video_queue = make_element("queue", "video-queue-x", &rtp_name);
    let audio_queue = make_element("queue", "audio-queue-x", &rtp_name);

    video_queue.set_property_from_str("leaky", &"true");
    audio_queue.set_property_from_str("leaky", &"true");

    let video_sink = make_element("proxysink", "rtp-video-sink", &rtp_name);
    let audio_sink = make_element("proxysink", "rtp-audio-sink", &rtp_name);

    let video_convert = make_element("videoconvert", "rtp-videoconvert", &rtp_name);

    let rtp_pipeline = gst::Pipeline::new("rtp-pipeline");
    rtp_pipeline.use_clock(&gst::SystemClock::obtain());

    // Add All The elements
    rtp_pipeline.add_many(&[&rtpjitterbuffer, &network_queue, &video_queue, &audio_queue, &audio_sink, &video_sink, &udpsrc, &decodebin]).unwrap();

    gst::Element::link_many(&[&udpsrc, &rtpjitterbuffer, &decodebin]).unwrap();
    gst::Element::link_many(&[&video_queue, &video_sink]).unwrap();
    gst::Element::link_many(&[&audio_queue, &audio_sink]).unwrap();

    let video_queue_weak = video_queue.downgrade();
    let audio_queue_weak = audio_queue.downgrade();
    let rtp_pipeline_weak = rtp_pipeline.downgrade();
    println!("Adding pad added handler");
    decodebin.connect_pad_added(move |_, src_pad| {
        let rtp_pipeline = match rtp_pipeline_weak.upgrade() {
            Some(rtp_pipeline) => rtp_pipeline,
            None => return,
        };
        println!("Inside pad added handler");
        let new_pad_caps = src_pad
            .get_current_caps()
            .expect("Failed to get caps of new pad.");
        let new_pad_struct = new_pad_caps
            .get_structure(0)
            .expect("Failed to get first structure of caps.");
        let new_pad_type = new_pad_struct.get_name();
        let is_audio = new_pad_type.starts_with("audio/x-raw");
        let is_video = new_pad_type.starts_with("video/x-raw");
        if is_audio {
            let audio_queue = match audio_queue_weak.upgrade() {
                Some(audio_queue) => audio_queue,
                None => return,
            };

            let sink_pad = audio_queue
                .get_static_pad("sink")
                .expect("Failed to get static sink pad from interaudiosink");
            if sink_pad.is_linked() {

                println!("We are already linked. Ignoring.");
                return;
            }

            let ret = src_pad.link(&sink_pad).unwrap();
        }

        if is_video {
            println!("New Pad is Video");
            let video_queue = match video_queue_weak.upgrade() {
                Some(video_queue) => video_queue,
                None => return,
            };

            let sink_pad = video_queue
                .get_static_pad("sink")
                .expect("Failed to get static sink pad from convert");
            if sink_pad.is_linked() {
                println!("We are already linked. Ignoring.");
                return;
            }

            let ret = src_pad.link(&sink_pad).unwrap();
        }
    });

    let ret = rtp_pipeline.set_state(gst::State::Playing);

    (audio_sink, video_sink, rtp_pipeline.get_base_time())
}

fn start_output_pipeline(audio_proxysink: gst::Element, video_proxysink: gst::Element, base_time: gst::ClockTime) {
    println!("Starting Output Pipeline... BaseTime: {:?}", base_time);
    let channel = "output-channel";
    let intervideosrc = make_element("proxysrc", "intervideosrc", &channel);
    intervideosrc.set_property("proxysink", &video_proxysink);


    let interaudiosrc = make_element("proxysrc", "interaudiosrc", &channel);
    interaudiosrc.set_property("proxysink", &audio_proxysink);

    let video_queue = make_element("queue", "rtp-video-queue", &channel);
    let audio_queue = make_element("queue", "rtp-audio-queue", &channel);

    let audio_convert = make_element("audioconvert", "audio-convert", &channel);
    let audio_resample = make_element("audioresample", "audio-resample", &channel);
    let video_convert = make_element("videoconvert", "video-convert", &channel);
    let video_scale = make_element("videoscale", "video-scale", &channel);
    let video_rate = make_element("videorate", "video-rate", &channel);
    let audio_capsfilter = make_element("capsfilter", "audio-capsfilter", &channel);
    let video_capsfilter = make_element("capsfilter", "video-capsfilter", &channel);
    let deinterlace = make_element("deinterlace", "video-deinterlace", &channel);

    let test_video_sink = make_element("osxvideosink", "test-video-sink", &channel);
    let test_audio_sink = make_element("autoaudiosink", "test-audio-sink", &channel);
    let rtp_pipeline = gst::Pipeline::new("test-rtp-pipeline");

    rtp_pipeline.use_clock(&gst::SystemClock::obtain());
    // Create and Set Caps. Right now everything is 1920x1080
    let video_caps = gst::Caps::builder("video/x-raw")
            .field("width", &1920)
            .field("height", &1080)
            //.field("format", &"BGRA")
            //.field("framerate", &gst::Fraction::new(30, 1))
            .any_features()
            .build();

    video_capsfilter.set_property("caps", &video_caps).unwrap();

    let audio_caps = gst::Caps::builder("audio/x-raw")
        .field("channels", &2)
        .field("rate", &48_000)
        .field("layout", &"interleaved")
        .any_features()
        .build();

    audio_capsfilter.set_property("caps", &audio_caps).unwrap();

    rtp_pipeline.add_many(&[&test_video_sink, &test_audio_sink, &audio_convert, &audio_resample, &video_convert, &video_rate, &video_scale, &audio_capsfilter, &video_capsfilter, &intervideosrc, &interaudiosrc, &video_queue, &audio_queue]).unwrap();

    gst::Element::link_many(&[&intervideosrc, &video_scale, &video_rate, &video_convert, &video_capsfilter, &video_queue, &test_video_sink]);
    gst::Element::link_many(&[&interaudiosrc, &audio_convert, &audio_resample, &audio_capsfilter, &audio_queue, &test_audio_sink]);

    rtp_pipeline.set_base_time(base_time);
    rtp_pipeline.set_state(gst::State::Playing);
}
