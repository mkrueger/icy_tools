#/bin/sh
cargo fmt --check
if [ $? -ne 0 ]; then 
  cargo fmt
  echo "commit formatting changes!"
  exit 1
fi

cargo test
if [ $? -ne 0 ]; then
  exit 1
fi
#git push 
