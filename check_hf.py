import urllib.request
import json
url = "https://huggingface.co/api/models/bartowski/gemma-4-E4B-it-GGUF/tree/main"
try:
    req = urllib.request.Request(url)
    with urllib.request.urlopen(req) as response:
        data = json.loads(response.read().decode())
        proj_files = [f['path'] for f in data if 'mmproj' in f['path'].lower()]
        print("FOUND PROJ:", proj_files)
except Exception as e:
    print("ERROR:", e)
