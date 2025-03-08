

use std::fs::File;
use std::io::Read;

use webp_animation::prelude::*;
use image::DynamicImage;

use std::path::Path;
use std::io::Write;

use image::ImageEncoder;

///
/// Watch a directory for webp files and convert them to mp4
/// 
#[tokio::main]
async fn main() {
    let args = std::env::args().collect::<Vec<String>>();
    if args.len() < 2 {
        println!("Usage: autowebp2mp4 <path>");
        return;
    }
    let path = &args[1];

    
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
    loop {
        let ctrlc = tokio::signal::ctrl_c();
        tokio::select! {
            _ = interval.tick() => ticked(path).await,
            _ = ctrlc => {
                break;
            }
        }
    }
}

async fn ticked<P: AsRef<Path>>(path: P) {
    let files = std::fs::read_dir(path).unwrap();
    for file in files {
        let file = file.unwrap();
        let file_path = file.path();
        let basename = file_path.file_stem().unwrap().to_string_lossy();
        if file_path.file_name().unwrap().to_string_lossy().ends_with(".webp") {
            if std::fs::exists(format!("{}.mp4", basename)).unwrap() {
                continue;
            }
            let filelen = file.metadata().unwrap().len();
            if filelen < 1000 {
                println!("File is too small: {} {} bytes", file_path.display(), filelen);
                continue;
            }
            println!("Found a webp file: {} {} bytes", file_path.display(), file.metadata().unwrap().len());

            convert_webp_to_mp4(&file_path, &basename).await;
            if std::fs::exists(format!("{}.mp4", &basename)).unwrap() {
                let file = std::fs::File::open(format!("{}.mp4", &basename)).unwrap();
                println!("Converted to mp4: {}.mp4 {} bytes", basename, file.metadata().unwrap().len());
            }
        }
    }
    println!("Ticked at {:?}", std::time::Instant::now());
}



#[derive(Debug)]
pub enum Error {
    Io(std::io::Error)
}

pub fn load_webp_file<P: AsRef<Path>>(path: P) -> Result<Vec<DynamicImage>, Error> {
    let mut file = File::open(path).map_err(Error::Io)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).map_err(Error::Io)?;
    match webp::Decoder::new(&buf).decode() {
        Some(img) => Ok(vec![img.to_image()]),
        None => {
            let decoder = Decoder::new(&buf).unwrap();
        
            Ok(decoder.into_iter().enumerate()
                // .inspect(|(i, frame)| {
                //     println!(
                //         "Frame-{:03}, dimensions={:?}, data_len={}",
                //         i,
                //         frame.dimensions(),
                //         frame.data().len()
                //     );
                // })
                .filter_map(|(_, frame)| {
                frame.into_image().ok()
                })
                .map(|x|DynamicImage::ImageRgba8(x))
                .collect::<Vec<_>>())
        }    
    }    
}

pub async fn convert_webp_to_mp4<P: AsRef<Path>, S: AsRef<str>>(path: P, basename: S) {

    let imgs = load_webp_file(path.as_ref());
        // .expect(&format!("Could not load webp file {}", path.as_ref().to_str().unwrap()));
    if imgs.is_err() {
        println!("Could not load webp file {}", path.as_ref().to_str().unwrap());
        return;
    }
    let imgs = imgs.unwrap();
    println!("Loaded {} images", imgs.len());
    let (w, h) = (imgs[0].width(), imgs[0].height());

    println!("{w} x {h}");
    println!("{} frames found", imgs.len());
    
    imgs.iter().enumerate().for_each(|(i, img)| {
        
        let _encoded = {
            let raw_bytes = img.to_rgba8().into_raw();
            let mut encoded = vec![];
            let cursor = std::io::Cursor::new(&mut encoded);
            image::codecs::png::PngEncoder::new(cursor)
                .write_image(&raw_bytes, img.width(), img.height(), image::ColorType::Rgba8)
                 //image::ExtendedColorType::Rgba8)
                .expect("Could not encode image");
            // write the encoded image to a file named frame001.png
            let mut file = std::fs::File::create(format!("frame{:03}.png", i)).expect("Could not create file");
            file.write_all(&encoded).expect("Could not write to file");
            encoded
        };
    });

    let outfile = format!("{}.mp4", basename.as_ref());
    // using ffmpeg we'll convert the png files to a video
    // ffmpeg -framerate 30 -i frame%03d.png -c:v libx264 -profile:v high -crf 20 -pix_fmt yuv420p output.mp4
    tokio::process::Command::new("ffmpeg")
        .args(&[
            "-y",
            "-framerate", "15",
            "-i", "frame%03d.png",
            "-c:v", "libx264",
            "-profile:v", "high",
            "-crf", "20",
            "-pix_fmt", "yuv420p",
            outfile.as_ref(),
        ])
        .output()
        .await
        .expect("Could not run ffmpeg");

    assert!(std::fs::exists(outfile).expect("Could not check file existence"));
    for i in 0..imgs.len() {
        std::fs::remove_file(format!("frame{:03}.png", i)).expect("Could not remove file");
    }
}
