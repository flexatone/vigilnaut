


examples = [
'''
Package      Site
zipp-3.7.0   /home/ariza/.env-ra-tech/lib/python3.8/site-packages
             /home/ariza/.env-argo/lib/python3.8/site-packages
             /home/ariza/.env-fsqf/lib/python3.8/site-packages
             /home/ariza/.env-sf/lib/python3.8/site-packages
zipp-3.8.0   /home/ariza/.env-sf-fwd/lib/python3.8/site-packages
zipp-3.15.0  /home/ariza/.env-gpub/lib/python3.8/site-packages
             /home/ariza/.env310-sfpyo/lib/python3.10/site-packages
             /home/ariza/.env-arraymap/lib/python3.8/site-packages
zipp-3.16.0  /home/ariza/.env311-fetter-bench/lib/python3.11/site-packages
zipp-3.16.2  /home/ariza/.env311-sf/lib/python3.11/site-packages
             /home/ariza/.env38/lib/python3.8/site-packages
             /home/ariza/.env-hyray/lib/python3.8/site-packages
zipp-3.17.0  /home/ariza/.env312-sf/lib/python3.12/site-packages
zipp-3.18.1  /home/ariza/.env311-tpc/lib/python3.11/site-packages
             /home/ariza/.env311-uv/lib/python3.11/site-packages
             /home/ariza/.env312-fetter-swppc/lib/python3.8/site-packages
zipp-3.20.2  /home/ariza/.env311/lib/python3.11/site-packages
''',

'''
$ fetter -e python3 scan
Package                   Site
certifi-2024.8.30         /home/ariza/.env312-fetter-swppc/lib/python3.8/site-packages
charset_normalizer-3.4.0  /home/ariza/.env312-fetter-swppc/lib/python3.8/site-packages
idna-3.10                 /home/ariza/.env312-fetter-swppc/lib/python3.8/site-packages
jinja2-3.1.3              /home/ariza/.env312-fetter-swppc/lib/python3.8/site-packages
markupsafe-2.1.5          /home/ariza/.env312-fetter-swppc/lib/python3.8/site-packages
pip-21.1.1                /home/ariza/.env312-fetter-swppc/lib/python3.8/site-packages
requests-2.32.3           /home/ariza/.env312-fetter-swppc/lib/python3.8/site-packages
setuptools-56.0.0         /home/ariza/.env312-fetter-swppc/lib/python3.8/site-packages
urllib3-2.2.3             /home/ariza/.env312-fetter-swppc/lib/python3.8/site-packages
zipp-3.18.1               /home/ariza/.env312-fetter-swppc/lib/python3.8/site-packages
''',


'''
$ fetter -e python3 validate --bound requirements.txt
Package                   Dependency  Explain     Sites
certifi-2024.8.30                     Unrequired  /home/ariza/.env312-fetter-swppc/lib/python3.8/site-packages
charset_normalizer-3.4.0              Unrequired  /home/ariza/.env312-fetter-swppc/lib/python3.8/site-packages
idna-3.10                             Unrequired  /home/ariza/.env312-fetter-swppc/lib/python3.8/site-packages
markupsafe-2.1.5                      Unrequired  /home/ariza/.env312-fetter-swppc/lib/python3.8/site-packages
pip-21.1.1                            Unrequired  /home/ariza/.env312-fetter-swppc/lib/python3.8/site-packages
setuptools-56.0.0                     Unrequired  /home/ariza/.env312-fetter-swppc/lib/python3.8/site-packages
urllib3-2.2.3                         Unrequired  /home/ariza/.env312-fetter-swppc/lib/python3.8/site-packages
''',

'''
$ fetter -e python3 validate --bound requirements.txt --superset
Package      Dependency    Explain     Sites
zipp-3.20.2  zipp==3.18.1  Misdefined  /home/ariza/.env312-fetter-swppc/lib/python3.8/site-packages
''',

'''
$ fetter search -p numpy-*
Package       Site
numpy-1.18.5  /home/ariza/.env-argo/lib/python3.8/site-packages
numpy-1.19.5  /home/ariza/.env-sf/lib/python3.8/site-packages
numpy-1.22.0  /home/ariza/.env-fsqf/lib/python3.8/site-packages
numpy-1.22.2  /home/ariza/.env310/lib/python3.10/site-packages
numpy-1.22.4  /home/ariza/.env38/lib/python3.8/site-packages
numpy-1.23.5  /home/ariza/.env-hyray/lib/python3.8/site-packages
              /home/ariza/.env39/lib/python3.9/site-packages
              /home/ariza/.env-gpub/lib/python3.8/site-packages
              /home/ariza/.env-arraymap/lib/python3.8/site-packages
              /home/ariza/.env311-ak/lib/python3.11/site-packages
              /home/ariza/.env-npb/lib/python3.10/site-packages
numpy-1.24.2  /home/ariza/.env311-er/lib/python3.11/site-packages
              /home/ariza/.env-sf-fwd/lib/python3.8/site-packages
              /home/ariza/.env-automap/lib/python3.8/site-packages
numpy-1.24.3  /home/ariza/.env311/lib/python3.11/site-packages
              /home/ariza/.env-ak/lib/python3.8/site-packages
              /home/ariza/.env311-uv/lib/python3.11/site-packages
numpy-1.24.4  /home/ariza/.env-ra-tech/lib/python3.8/site-packages
numpy-1.25.1  /home/ariza/.env311-sf/lib/python3.11/site-packages
numpy-1.26.0  /home/ariza/.env311-fetter-bench/lib/python3.11/site-packages
numpy-1.26.2  /home/ariza/.env312/lib/python3.12/site-packages
              /home/ariza/.env312-arraymap/lib/python3.12/site-packages
numpy-1.26.4  /home/ariza/.env311-sage/lib/python3.11/site-packages
              /home/ariza/.env312-dft/lib/python3.12/site-packages
numpy-2.0.0   /home/ariza/.env312-ak/lib/python3.12/site-packages
              /home/ariza/.env312-sf/lib/python3.12/site-packages
numpy-2.1.2   /home/ariza/.env311-test/lib/python3.11/site-packages
''',

'''
fetter unpack-count -p numpy-1.18.5
Package       Site                                   Files  Dirs
numpy-1.18.5  ~/.env-ag/lib/python3.8/site-packages  855    2
''',

'''
Package|Site
numpy-1.19.5|/home/ariza/.env-sf/lib/python3.8/site-packages
numpy-1.22.0|/home/ariza/.env-fsqf/lib/python3.8/site-packages
numpy-1.22.2|/home/ariza/.env310/lib/python3.10/site-packages
numpy-1.22.4|/home/ariza/.env38/lib/python3.8/site-packages
numpy-1.23.5|/home/ariza/.env-npb/lib/python3.10/site-packages
numpy-1.23.5|/home/ariza/.env-arraymap/lib/python3.8/site-packages
numpy-1.23.5|/home/ariza/.env-gpub/lib/python3.8/site-packages
numpy-1.23.5|/home/ariza/.env-hyray/lib/python3.8/site-packages
numpy-1.23.5|/home/ariza/.env39/lib/python3.9/site-packages
numpy-1.23.5|/home/ariza/.env311-ak/lib/python3.11/site-packages
numpy-1.24.2|/home/ariza/.env-automap/lib/python3.8/site-packages
numpy-1.24.2|/home/ariza/.env311-er/lib/python3.11/site-packages
numpy-1.24.2|/home/ariza/.env-sf-fwd/lib/python3.8/site-packages
numpy-1.24.3|/home/ariza/.env311/lib/python3.11/site-packages
numpy-1.24.3|/home/ariza/.env-ak/lib/python3.8/site-packages
numpy-1.24.3|/home/ariza/.env311-uv/lib/python3.11/site-packages
numpy-1.24.4|/home/ariza/.env-ra-tech/lib/python3.8/site-packages
numpy-1.25.1|/home/ariza/.env311-sf/lib/python3.11/site-packages
numpy-1.26.0|/home/ariza/.env311-fetter-bench/lib/python3.11/site-packages
numpy-1.26.2|/home/ariza/.env312-arraymap/lib/python3.12/site-packages
numpy-1.26.2|/home/ariza/.env312/lib/python3.12/site-packages
numpy-1.26.4|/home/ariza/.env312-dft/lib/python3.12/site-packages
numpy-1.26.4|/home/ariza/.env311-sage/lib/python3.11/site-packages
numpy-2.0.0|/home/ariza/.env312-ak/lib/python3.12/site-packages
numpy-2.0.0|/home/ariza/.env312-sf/lib/python3.12/site-packages
numpy-2.1.2|/home/ariza/.env311-test/lib/python3.11/site-packages
'''

]


from_to = {
    '/home/ariza/':         '~/',
    '.env-sf':              '.env-qa',
    '.env-ra-tech':         '.env-rt',
    '.env-argo':            '.env-ag',
    '.env-sf-fwd':          '.env-ff',
    '.env-arraymap':        '.env-yp',
    '.env-gpub':            '.env-gp',
    '.env310-sfpyo':        '.env-po',
    '.env311-fetter-bench': '.env-fb',
    '.env311-sf':           '.env-sf',
    '.env38':               '.env-te',
    '.env39':               '.env-tn',
    '.env-hyray':           '.env-hy',
    '.env312-sf':           '.env-sq',
    '.env311-tpc':          '.env-tp',
    '.env311-uv':           '.env-uv',
    '.env311':              '.env-tl',
    '.env312':              '.env-tt',
    '.env312-fetter-swppc': '.env-wp',
    '.env-fsqf':            '.env-qf',
    '.env-qa-fwd':          '.env-aw',
    '.env-automap':         '.env-am',
    '.env312-arraymap':     '.env-ma',
    '.env-tl-sage':         '.env-sg',
    '.env312-dft':          '.env-ft',
    '.env312-ak':           '.env-ak',
    '.env-tl-test':         '.env-lt',
    '.env-npb':             '.env-np',
    '.env-tl-er':           '.env-er',
    '.env-tl-ak':           '.env-tl',
    '.env-tt-arraymap':     '.env-rr',
    '.env-tt-dft':          '.env-df',
    '.env-tt-ak':           '.env-tt',
    }

def proc():
    for example in examples:
        for src, dst in from_to.items():
            example = example.replace(src, dst)

        print(example)
        print('--')


if __name__ == '__main__':
    proc()