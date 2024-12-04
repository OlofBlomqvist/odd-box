docker build -f Dockerfile.build -t odd-box-builder .
docker create --name builder odd-box-builder
mkdir -p target
docker cp builder:/usr/src/odd-box/target/x86_64-unknown-linux-musl/release/odd-box ./target/odd-box-static
docker rm builder
