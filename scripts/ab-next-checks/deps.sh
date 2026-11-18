# sourced by run_ab.sh: PyYAML + Pillow on PATH's python3, as the job's venv gives it
if python3 -c 'import yaml, PIL' 2>/dev/null; then return 0; fi
d="$(mktemp -d)"
if python3 -m venv "$d/venv" 2>/dev/null && "$d/venv/bin/python" -m pip install -q --disable-pip-version-check PyYAML==6.0.3 Pillow==11.3.0; then
  export PATH="$d/venv/bin:$PATH"
else
  python3 -m pip install -q --disable-pip-version-check --target "$d/site" PyYAML==6.0.3 Pillow==11.3.0 2>/dev/null \
    || python3 -m pip install -q --break-system-packages --target "$d/site" PyYAML==6.0.3 Pillow==11.3.0 2>/dev/null \
    || { command -v uv >/dev/null && uv pip install -q --target "$d/site" PyYAML==6.0.3 Pillow==11.3.0; }
  export PYTHONPATH="$d/site${PYTHONPATH:+:$PYTHONPATH}"
fi
python3 -c 'import yaml, PIL; print("deps ok", yaml.__version__)' || echo "harness: deps missing"
