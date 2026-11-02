use std::collections::{BTreeSet, HashSet};
use std::hint::black_box;
use std::time::Instant;

const fn classes() -> [u8; 256] {
    let mut classes = [0; 256];
    let mut i = 0;
    while i < 256 {
        let b = i as u8;
        classes[i] = if matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'_') { 1 }
            else if matches!(b, b'0'..=b'9') { 2 }
            else if matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0c) { 3 }
            else if b == b'.' { 4 }
            else { 0 };
        i += 1;
    }
    classes
}
const CLASSES: [u8;256] = classes();

// MODE 0 is the original predicate reference; 1 is current production;
// 2 replaces all ASCII predicates; 3 classifies through one constant table.
fn scan<'a, const MODE: u8>(text: &'a str, mut visit: impl FnMut(&'a str)) {
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut after_dot = false;
    while i < bytes.len() {
        let b = bytes[i];
        let start_ident = if MODE == 3 { CLASSES[b as usize] == 1 }
            else if MODE != 0 { matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'_') }
            else { b.is_ascii_alphabetic() || b == b'_' };
        if start_ident {
            let start = i;
            while i < bytes.len() && {
                let b = bytes[i];
                if MODE == 3 { matches!(CLASSES[b as usize], 1 | 2) }
                else if MODE != 0 { matches!(b,b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_') }
                else { b.is_ascii_alphanumeric() || b == b'_' }
            } { i += 1; }
            if i-start == 1 && b == b'r' && bytes.get(i) == Some(&b'#') { i += 1; continue; }
            if !after_dot { visit(&text[start..i]); }
            after_dot = false;
        } else if if MODE == 3 { CLASSES[b as usize] == 2 }
                  else if MODE == 2 { matches!(b,b'0'..=b'9') }
                  else { b.is_ascii_digit() } {
            while i < bytes.len() && {
                let b=bytes[i];
                if MODE == 3 { matches!(CLASSES[b as usize],1 | 2) }
                else if MODE != 0 { matches!(b,b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_') }
                else { b.is_ascii_alphanumeric() || b == b'_' }
            } { i+=1; }
            after_dot=false;
        } else if b == b'.' {
            if bytes.get(i+1) == Some(&b'.') { i+=2; after_dot=false; }
            else { i+=1; after_dot=true; }
        } else if if MODE == 3 { CLASSES[b as usize] == 3 }
                  else if MODE == 2 { matches!(b,b' ' | b'\t' | b'\n' | b'\r' | 0x0c) }
                  else { b.is_ascii_whitespace() } { i+=1; }
        else { i+=1; after_dot=false; }
    }
}
fn names<const MODE:u8>(text:&str)->BTreeSet<&str>{
    let mut set=HashSet::new();
    scan::<MODE>(text,|name|{set.insert(name);});
    set.into_iter().collect()
}
fn run(variant:usize,text:&str)->BTreeSet<&str>{match variant {
    0=>names::<1>(text),1=>names::<2>(text),2=>names::<3>(text),_=>unreachable!()
}}
fn main(){
    let source=std::env::args().nth(1).unwrap();
    let root=std::fs::read_to_string(source).unwrap();
    let text=root.repeat(10);
    let edges=[
        "r#type r#match foo.r#method . field ..Range 1..pump 0xff_suffix 1e9_tail",
        "// comment shout\n\"literal Handler\" /*Mode*/ name .\n method; name..pump",
        "r# r## a...b a.0.foo 12abc_def _0 _a A9 9foo . r#field, r#type",
        "éshout λHandler 中文Mode café 🦀Line .éfoo ..λbar",
        "a.\tfoo a.\nfoo a.\rfoo a.\u{000c}foo a.\u{000b}foo a.#foo",
    ];
    for input in std::iter::once(root.as_str()).chain(edges){
        let mut reference=Vec::new(); scan::<0>(input,|name|reference.push(name));
        let mut v1=Vec::new();scan::<1>(input,|name|v1.push(name));assert_eq!(reference,v1);
        let mut v2=Vec::new();scan::<2>(input,|name|v2.push(name));assert_eq!(reference,v2);
        let mut v3=Vec::new();scan::<3>(input,|name|v3.push(name));assert_eq!(reference,v3);
    }
    for b in 0u8..=255{
        assert_eq!(b.is_ascii_alphabetic()||b==b'_',CLASSES[b as usize]==1);
        assert_eq!(b.is_ascii_alphanumeric()||b==b'_',matches!(CLASSES[b as usize],1|2));
        assert_eq!(b.is_ascii_digit(),matches!(b,b'0'..=b'9'));
        assert_eq!(b.is_ascii_digit(),CLASSES[b as usize]==2);
        assert_eq!(b.is_ascii_whitespace(),matches!(b,b' ' | b'\t' | b'\n' | b'\r' | 0x0c));
        assert_eq!(b.is_ascii_whitespace(),CLASSES[b as usize]==3);
    }
    let expected=run(0,&text);
    println!("bytes={} unique={} all occurrence sequences and256classifications equal",text.len(),expected.len());
    let labels=["direct-partial","direct-all","class-table"];
    for pass in 0..5{
        for offset in 0..3{
            let variant=if pass%2==0{offset}else{2-offset};
            let start=Instant::now();let result=run(variant,black_box(&text));let elapsed=start.elapsed().as_secs_f64();
            assert_eq!(expected,result);black_box(&result);
            println!("pass={} variant={} seconds={elapsed:.9}",pass+1,labels[variant]);
        }
    }
}
