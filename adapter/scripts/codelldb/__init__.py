import logging
logging.basicConfig(level=logging.DEBUG, #filename='/tmp/codelldb.log',
                    format='%(levelname)s(Python) %(asctime)s %(name)s: %(message)s', datefmt='%H:%M:%S')

from .codelldb import initialize
