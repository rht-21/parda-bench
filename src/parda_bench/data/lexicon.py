"""Word lists for synthetic people and places, each in Latin script and Devanagari."""

from typing import NamedTuple


class Bilingual(NamedTuple):
    latin: str
    deva: str


class City(NamedTuple):
    name: Bilingual
    state: Bilingual
    pin_prefix: str
    """First three digits of the city's PIN codes."""


FIRST_NAMES = (
    Bilingual("Aarav", "आरव"),
    Bilingual("Vivaan", "विवान"),
    Bilingual("Aditya", "आदित्य"),
    Bilingual("Arjun", "अर्जुन"),
    Bilingual("Rohan", "रोहन"),
    Bilingual("Rahul", "राहुल"),
    Bilingual("Amit", "अमित"),
    Bilingual("Vikram", "विक्रम"),
    Bilingual("Sanjay", "संजय"),
    Bilingual("Rajesh", "राजेश"),
    Bilingual("Suresh", "सुरेश"),
    Bilingual("Manoj", "मनोज"),
    Bilingual("Deepak", "दीपक"),
    Bilingual("Karan", "करण"),
    Bilingual("Nikhil", "निखिल"),
    Bilingual("Pranav", "प्रणव"),
    Bilingual("Ishaan", "ईशान"),
    Bilingual("Kabir", "कबीर"),
    Bilingual("Siddharth", "सिद्धार्थ"),
    Bilingual("Harsh", "हर्ष"),
    Bilingual("Priya", "प्रिया"),
    Bilingual("Ananya", "अनन्या"),
    Bilingual("Diya", "दीया"),
    Bilingual("Kavya", "काव्या"),
    Bilingual("Neha", "नेहा"),
    Bilingual("Pooja", "पूजा"),
    Bilingual("Sneha", "स्नेहा"),
    Bilingual("Anjali", "अंजलि"),
    Bilingual("Meera", "मीरा"),
    Bilingual("Riya", "रिया"),
    Bilingual("Sunita", "सुनीता"),
    Bilingual("Lakshmi", "लक्ष्मी"),
    Bilingual("Divya", "दिव्या"),
    Bilingual("Shreya", "श्रेया"),
    Bilingual("Aishwarya", "ऐश्वर्या"),
    Bilingual("Nandini", "नंदिनी"),
    Bilingual("Swati", "स्वाति"),
    Bilingual("Kiran", "किरण"),
    Bilingual("Geeta", "गीता"),
    Bilingual("Farhan", "फ़रहान"),
    Bilingual("Imran", "इमरान"),
    Bilingual("Ayesha", "आयशा"),
    Bilingual("Zoya", "ज़ोया"),
    Bilingual("Gurpreet", "गुरप्रीत"),
    Bilingual("Harpreet", "हरप्रीत"),
    Bilingual("Joseph", "जोसेफ"),
    Bilingual("Mary", "मैरी"),
    Bilingual("Venkatesh", "वेंकटेश"),
    Bilingual("Lakshman", "लक्ष्मण"),
    Bilingual("Murugan", "मुरुगन"),
)

SURNAMES = (
    Bilingual("Sharma", "शर्मा"),
    Bilingual("Verma", "वर्मा"),
    Bilingual("Gupta", "गुप्ता"),
    Bilingual("Singh", "सिंह"),
    Bilingual("Kumar", "कुमार"),
    Bilingual("Patel", "पटेल"),
    Bilingual("Shah", "शाह"),
    Bilingual("Mehta", "मेहता"),
    Bilingual("Iyer", "अय्यर"),
    Bilingual("Nair", "नायर"),
    Bilingual("Reddy", "रेड्डी"),
    Bilingual("Rao", "राव"),
    Bilingual("Menon", "मेनन"),
    Bilingual("Pillai", "पिल्लै"),
    Bilingual("Das", "दास"),
    Bilingual("Banerjee", "बनर्जी"),
    Bilingual("Chatterjee", "चटर्जी"),
    Bilingual("Mukherjee", "मुखर्जी"),
    Bilingual("Joshi", "जोशी"),
    Bilingual("Kulkarni", "कुलकर्णी"),
    Bilingual("Deshpande", "देशपांडे"),
    Bilingual("Patil", "पाटिल"),
    Bilingual("Yadav", "यादव"),
    Bilingual("Mishra", "मिश्रा"),
    Bilingual("Tiwari", "तिवारी"),
    Bilingual("Pandey", "पांडे"),
    Bilingual("Chauhan", "चौहान"),
    Bilingual("Rathore", "राठौर"),
    Bilingual("Agarwal", "अग्रवाल"),
    Bilingual("Jain", "जैन"),
    Bilingual("Khan", "ख़ान"),
    Bilingual("Ansari", "अंसारी"),
    Bilingual("Qureshi", "क़ुरैशी"),
    Bilingual("Fernandes", "फ़र्नांडिस"),
    Bilingual("D'Souza", "डिसूज़ा"),
    Bilingual("Gill", "गिल"),
    Bilingual("Sandhu", "संधू"),
    Bilingual("Naidu", "नायडू"),
    Bilingual("Bhat", "भट"),
    Bilingual("Saxena", "सक्सेना"),
)

LOCALITIES = (
    Bilingual("MG Road", "एमजी रोड"),
    Bilingual("Station Road", "स्टेशन रोड"),
    Bilingual("Shastri Nagar", "शास्त्री नगर"),
    Bilingual("Civil Lines", "सिविल लाइंस"),
    Bilingual("Model Town", "मॉडल टाउन"),
    Bilingual("Rajendra Nagar", "राजेंद्र नगर"),
    Bilingual("Sector 15", "सेक्टर 15"),
    Bilingual("Indira Colony", "इंदिरा कॉलोनी"),
    Bilingual("Laxmi Nagar", "लक्ष्मी नगर"),
    Bilingual("Park Street", "पार्क स्ट्रीट"),
    Bilingual("Ashok Vihar", "अशोक विहार"),
    Bilingual("Malviya Nagar", "मालवीय नगर"),
)

BUILDINGS = (
    Bilingual("Sai Krupa Apartments", "साई कृपा अपार्टमेंट"),
    Bilingual("Shanti Niwas", "शांति निवास"),
    Bilingual("Green Park Society", "ग्रीन पार्क सोसाइटी"),
    Bilingual("Lotus Residency", "लोटस रेजीडेंसी"),
    Bilingual("Ganga Heights", "गंगा हाइट्स"),
    Bilingual("Krishna Kunj", "कृष्ण कुंज"),
)

LANDMARKS = (
    Bilingual("Hanuman Mandir", "हनुमान मंदिर"),
    Bilingual("City Hospital", "सिटी अस्पताल"),
    Bilingual("Bus Stand", "बस स्टैंड"),
    Bilingual("Post Office", "डाकघर"),
    Bilingual("Government School", "सरकारी स्कूल"),
)

CITIES = (
    City(Bilingual("Mumbai", "मुंबई"), Bilingual("Maharashtra", "महाराष्ट्र"), "400"),
    City(Bilingual("Pune", "पुणे"), Bilingual("Maharashtra", "महाराष्ट्र"), "411"),
    City(Bilingual("Nagpur", "नागपुर"), Bilingual("Maharashtra", "महाराष्ट्र"), "440"),
    City(Bilingual("New Delhi", "नई दिल्ली"), Bilingual("Delhi", "दिल्ली"), "110"),
    City(Bilingual("Bengaluru", "बेंगलुरु"), Bilingual("Karnataka", "कर्नाटक"), "560"),
    City(Bilingual("Chennai", "चेन्नई"), Bilingual("Tamil Nadu", "तमिलनाडु"), "600"),
    City(Bilingual("Hyderabad", "हैदराबाद"), Bilingual("Telangana", "तेलंगाना"), "500"),
    City(Bilingual("Kolkata", "कोलकाता"), Bilingual("West Bengal", "पश्चिम बंगाल"), "700"),
    City(Bilingual("Ahmedabad", "अहमदाबाद"), Bilingual("Gujarat", "गुजरात"), "380"),
    City(Bilingual("Jaipur", "जयपुर"), Bilingual("Rajasthan", "राजस्थान"), "302"),
    City(Bilingual("Lucknow", "लखनऊ"), Bilingual("Uttar Pradesh", "उत्तर प्रदेश"), "226"),
    City(Bilingual("Varanasi", "वाराणसी"), Bilingual("Uttar Pradesh", "उत्तर प्रदेश"), "221"),
    City(Bilingual("Patna", "पटना"), Bilingual("Bihar", "बिहार"), "800"),
    City(Bilingual("Bhopal", "भोपाल"), Bilingual("Madhya Pradesh", "मध्य प्रदेश"), "462"),
    City(Bilingual("Indore", "इंदौर"), Bilingual("Madhya Pradesh", "मध्य प्रदेश"), "452"),
    City(Bilingual("Kochi", "कोच्चि"), Bilingual("Kerala", "केरल"), "682"),
    City(Bilingual("Chandigarh", "चंडीगढ़"), Bilingual("Chandigarh", "चंडीगढ़"), "160"),
)

NAMED_PLACES = (
    # Places and institutions named after people: text that contains a name but is not personal data.
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
)
