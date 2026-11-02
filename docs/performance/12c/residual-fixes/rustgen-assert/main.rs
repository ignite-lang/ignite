use std::hint::black_box;
use std::time::Instant;

fn original(line: &str) -> usize {
    line.matches(" + ").count() + line.matches(" && ").count()
}

fn prefiltered(line: &str) -> usize {
    let plus = if line.contains('+') { line.matches(" + ").count() } else { 0 };
    let amp = if line.contains('&') { line.matches(" && ").count() } else { 0 };
    plus + amp
}

fn verify(text: &str, prefilter: bool) -> Vec<usize> {
    text.lines().map(|line| if prefilter { prefiltered(line) } else { original(line) }).collect()
}

fn main() {
    let path = std::env::args().nth(1).expect("actual generated10k root");
    let root = std::fs::read_to_string(path).unwrap();
    let text = root.repeat(10);
    let edges = [
        "", "a", "+", "&", "&&", " + ", " && ", "x+y", "a&&b", " + + ",
        " && && ", " +  + ", " &&  && ", " +  &&  + ", " + \n && ",
        "é + 中文 && 🦀", "+++++ &&&&&", "\t+\t &&\n +\r\n", " + ".repeat(64).leak(),
    ];
    for edge in edges { assert_eq!(verify(edge,false),verify(edge,true),"edge {edge:?}"); }
    // Enumerate every short string of marker alphabet. Match counts preserve
    // exact spaces, nonoverlap, and malformed adjacent operators.
    let alphabet = [b' ', b'+', b'&', b'x'];
    for length in 0..=7 {
        for mut index in 0..4usize.pow(length) {
            let mut bytes = vec![0;length as usize];
            for b in &mut bytes { *b=alphabet[index%4];index/=4; }
            let edge=std::str::from_utf8(&bytes).unwrap();
            assert_eq!(original(edge),prefiltered(edge));
        }
    }
    let expected=verify(&text,false);
    assert_eq!(expected,verify(&text,true));
    println!("bytes={} lines={} plus_bytes={} amp_bytes={} all actual-line/exhaustive edge counts equal",text.len(),expected.len(),text.bytes().filter(|b|*b==b'+').count(),text.bytes().filter(|b|*b==b'&').count());
    for pass in 0..5 {
        for offset in 0..2 {
            let prefilter=(offset+(pass%2))%2==1;
            let start=Instant::now();
            let counts=verify(black_box(&text),prefilter);
            let elapsed=start.elapsed().as_secs_f64();
            assert_eq!(expected,counts);black_box(&counts);
            println!("pass={} variant={} seconds={elapsed:.9}",pass+1,if prefilter{"prefiltered"}else{"original"});
        }
    }
}
