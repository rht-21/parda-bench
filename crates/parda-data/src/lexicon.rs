//! Word lists for synthetic people and places, each in Latin script and Devanagari.

/// One word or phrase written in both scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bilingual {
    pub latin: &'static str,
    pub deva: &'static str,
}

const fn b(latin: &'static str, deva: &'static str) -> Bilingual {
    Bilingual { latin, deva }
}

pub const FIRST_NAMES: [Bilingual; 50] = [
    b("Aarav", "आरव"),
    b("Vivaan", "विवान"),
    b("Aditya", "आदित्य"),
    b("Arjun", "अर्जुन"),
    b("Rohan", "रोहन"),
    b("Rahul", "राहुल"),
    b("Amit", "अमित"),
    b("Vikram", "विक्रम"),
    b("Sanjay", "संजय"),
    b("Rajesh", "राजेश"),
    b("Suresh", "सुरेश"),
    b("Manoj", "मनोज"),
    b("Deepak", "दीपक"),
    b("Karan", "करण"),
    b("Nikhil", "निखिल"),
    b("Pranav", "प्रणव"),
    b("Ishaan", "ईशान"),
    b("Kabir", "कबीर"),
    b("Siddharth", "सिद्धार्थ"),
    b("Harsh", "हर्ष"),
    b("Priya", "प्रिया"),
    b("Ananya", "अनन्या"),
    b("Diya", "दीया"),
    b("Kavya", "काव्या"),
    b("Neha", "नेहा"),
    b("Pooja", "पूजा"),
    b("Sneha", "स्नेहा"),
    b("Anjali", "अंजलि"),
    b("Meera", "मीरा"),
    b("Riya", "रिया"),
    b("Sunita", "सुनीता"),
    b("Lakshmi", "लक्ष्मी"),
    b("Divya", "दिव्या"),
    b("Shreya", "श्रेया"),
    b("Aishwarya", "ऐश्वर्या"),
    b("Nandini", "नंदिनी"),
    b("Swati", "स्वाति"),
    b("Kiran", "किरण"),
    b("Geeta", "गीता"),
    b("Farhan", "फ़रहान"),
    b("Imran", "इमरान"),
    b("Ayesha", "आयशा"),
    b("Zoya", "ज़ोया"),
    b("Gurpreet", "गुरप्रीत"),
    b("Harpreet", "हरप्रीत"),
    b("Joseph", "जोसेफ"),
    b("Mary", "मैरी"),
    b("Venkatesh", "वेंकटेश"),
    b("Lakshman", "लक्ष्मण"),
    b("Murugan", "मुरुगन"),
];

pub const SURNAMES: [Bilingual; 40] = [
    b("Sharma", "शर्मा"),
    b("Verma", "वर्मा"),
    b("Gupta", "गुप्ता"),
    b("Singh", "सिंह"),
    b("Kumar", "कुमार"),
    b("Patel", "पटेल"),
    b("Shah", "शाह"),
    b("Mehta", "मेहता"),
    b("Iyer", "अय्यर"),
    b("Nair", "नायर"),
    b("Reddy", "रेड्डी"),
    b("Rao", "राव"),
    b("Menon", "मेनन"),
    b("Pillai", "पिल्लै"),
    b("Das", "दास"),
    b("Banerjee", "बनर्जी"),
    b("Chatterjee", "चटर्जी"),
    b("Mukherjee", "मुखर्जी"),
    b("Joshi", "जोशी"),
    b("Kulkarni", "कुलकर्णी"),
    b("Deshpande", "देशपांडे"),
    b("Patil", "पाटिल"),
    b("Yadav", "यादव"),
    b("Mishra", "मिश्रा"),
    b("Tiwari", "तिवारी"),
    b("Pandey", "पांडे"),
    b("Chauhan", "चौहान"),
    b("Rathore", "राठौर"),
    b("Agarwal", "अग्रवाल"),
    b("Jain", "जैन"),
    b("Khan", "ख़ान"),
    b("Ansari", "अंसारी"),
    b("Qureshi", "क़ुरैशी"),
    b("Fernandes", "फ़र्नांडिस"),
    b("D'Souza", "डिसूज़ा"),
    b("Gill", "गिल"),
    b("Sandhu", "संधू"),
    b("Naidu", "नायडू"),
    b("Bhat", "भट"),
    b("Saxena", "सक्सेना"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct City {
    pub name: Bilingual,
    pub state: Bilingual,
    /// First three digits of the city's PIN codes.
    pub pin_prefix: &'static str,
}

const fn city(name: Bilingual, state: Bilingual, pin_prefix: &'static str) -> City {
    City {
        name,
        state,
        pin_prefix,
    }
}

const MAHARASHTRA: Bilingual = b("Maharashtra", "महाराष्ट्र");
const UTTAR_PRADESH: Bilingual = b("Uttar Pradesh", "उत्तर प्रदेश");
const MADHYA_PRADESH: Bilingual = b("Madhya Pradesh", "मध्य प्रदेश");

pub const CITIES: [City; 17] = [
    city(b("Mumbai", "मुंबई"), MAHARASHTRA, "400"),
    city(b("Pune", "पुणे"), MAHARASHTRA, "411"),
    city(b("Nagpur", "नागपुर"), MAHARASHTRA, "440"),
    city(b("New Delhi", "नई दिल्ली"), b("Delhi", "दिल्ली"), "110"),
    city(b("Bengaluru", "बेंगलुरु"), b("Karnataka", "कर्नाटक"), "560"),
    city(b("Chennai", "चेन्नई"), b("Tamil Nadu", "तमिलनाडु"), "600"),
    city(b("Hyderabad", "हैदराबाद"), b("Telangana", "तेलंगाना"), "500"),
    city(
        b("Kolkata", "कोलकाता"),
        b("West Bengal", "पश्चिम बंगाल"),
        "700",
    ),
    city(b("Ahmedabad", "अहमदाबाद"), b("Gujarat", "गुजरात"), "380"),
    city(b("Jaipur", "जयपुर"), b("Rajasthan", "राजस्थान"), "302"),
    city(b("Lucknow", "लखनऊ"), UTTAR_PRADESH, "226"),
    city(b("Varanasi", "वाराणसी"), UTTAR_PRADESH, "221"),
    city(b("Patna", "पटना"), b("Bihar", "बिहार"), "800"),
    city(b("Bhopal", "भोपाल"), MADHYA_PRADESH, "462"),
    city(b("Indore", "इंदौर"), MADHYA_PRADESH, "452"),
    city(b("Kochi", "कोच्चि"), b("Kerala", "केरल"), "682"),
    city(b("Chandigarh", "चंडीगढ़"), b("Chandigarh", "चंडीगढ़"), "160"),
];

pub const LOCALITIES: [Bilingual; 12] = [
    b("MG Road", "एमजी रोड"),
    b("Station Road", "स्टेशन रोड"),
    b("Shastri Nagar", "शास्त्री नगर"),
    b("Civil Lines", "सिविल लाइंस"),
    b("Model Town", "मॉडल टाउन"),
    b("Rajendra Nagar", "राजेंद्र नगर"),
    b("Sector 15", "सेक्टर 15"),
    b("Indira Colony", "इंदिरा कॉलोनी"),
    b("Laxmi Nagar", "लक्ष्मी नगर"),
    b("Park Street", "पार्क स्ट्रीट"),
    b("Ashok Vihar", "अशोक विहार"),
    b("Malviya Nagar", "मालवीय नगर"),
];

pub const BUILDINGS: [Bilingual; 6] = [
    b("Sai Krupa Apartments", "साई कृपा अपार्टमेंट"),
    b("Shanti Niwas", "शांति निवास"),
    b("Green Park Society", "ग्रीन पार्क सोसाइटी"),
    b("Lotus Residency", "लोटस रेजीडेंसी"),
    b("Ganga Heights", "गंगा हाइट्स"),
    b("Krishna Kunj", "कृष्ण कुंज"),
];

pub const LANDMARKS: [Bilingual; 5] = [
    b("Hanuman Mandir", "हनुमान मंदिर"),
    b("City Hospital", "सिटी अस्पताल"),
    b("Bus Stand", "बस स्टैंड"),
    b("Post Office", "डाकघर"),
    b("Government School", "सरकारी स्कूल"),
];

/// Places and institutions named after people: text that contains a person's name but is not personal data.
pub const NAMED_PLACES: [&str; 12] = [
    "Gandhi Nagar",
    "Nehru Place",
    "Rajiv Chowk",
    "Indira Gandhi International Airport",
    "Sardar Patel Stadium",
    "Jawaharlal Nehru University",
    "Lal Bahadur Shastri Marg",
    "Subhas Chandra Bose Road",
    "Sarojini Nagar",
    "Chhatrapati Shivaji Maharaj Terminus",
    "Birla Mandir",
    "Tata Memorial Hospital",
];
