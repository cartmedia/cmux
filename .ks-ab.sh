set -e
cd webviews
rm -rf ../wv-base && mkdir ../wv-base
for f in $(ls -A | grep -v '^node_modules$'); do cp -R "$f" ../wv-base/; done
ln -s "$PWD/node_modules" ../wv-base/node_modules
(cd ../wv-base && patch -p2 < ../.ks-base.patch)
diff -rq ../wv-base/src src || true
bun bench/composer-typing.bench.ts --arms base=../wv-base,key=. --lengths 1500 --modes stream,idle --keys 60 --rounds 4 2>&1 | grep -vE '^\s*$' | tail -90
