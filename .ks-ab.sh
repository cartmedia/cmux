set -e
cd webviews
BASE=b1b05cf4e16c1c1f8d6333ba9bd9e490ee3abffe
git cat-file -e $BASE^{commit} 2>/dev/null || git fetch -q --depth=1 origin $BASE
rm -rf ../wv-base && mkdir ../wv-base
for f in $(ls -A | grep -v '^node_modules$'); do cp -R "$f" ../wv-base/; done
ln -s "$PWD/node_modules" ../wv-base/node_modules
rm -rf ../wv-base/src && mkdir -p /tmp/wvb && git archive $BASE webviews/src | tar -x -C /tmp/wvb && mv /tmp/wvb/webviews/src ../wv-base/src
diff -rq ../wv-base/src src | head -5 || true
export CMUX_PLAYWRIGHT_BUNDLED_CHROMIUM=1
bun bench/composer-typing.bench.ts --arms base=../wv-base,key=. --lengths 1500 --modes stream,idle --keys 60 --rounds 4 2>&1 | grep -vE '^\s*$' | tail -90
