"Shared fixtures: texts and words from laghu's offline corpus, plus Sanskrit in both scripts."
import re, pytest

SANSKRIT = ('धर्मक्षेत्रे कुरुक्षेत्रे समवेता युयुत्सवः । मामकाः पाण्डवाश्चैव किमकुर्वत सञ्जय ॥ '
            'ईशा वास्यमिदꣳ सर्वं यत्किञ्च जगत्यां जगत् । अ॒ग्निमी॑ळे पु॒रोहि॑तं य॒ज्ञस्य॑ दे॒वमृ॒त्विज॑म् ॥ '
            'dharmakṣetre kurukṣetre samavetā yuyutsavaḥ Bhīṣma Bheeshma Krishna Kṛṣṇa Lakshmana Lakṣmaṇa '
            'Raghuvamsha Raghuvaṃśa saundarya soundarya Śaṅkarācārya Shankaracharya ज़मीन फ़ारसी ड़ ०१२ ॐ ऋषि ṛṣi duties 02 STRASSE Straße')

@pytest.fixture(scope='session')
def texts():
    laghu = pytest.importorskip('laghu')
    return laghu.corpus() + [SANSKRIT]

@pytest.fixture(scope='session')
def words(texts):
    return sorted({w for t in texts for w in re.findall(r'\w+', t)} | set(SANSKRIT.split()))
