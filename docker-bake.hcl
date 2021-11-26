target "docker-metadata-action" {}

target "build" {
  inherits = ["docker-metadata-action"]
  tags = ["docker.io/username/webapp"]
  context = "./"
  dockerfile = "Dockerfile"
  platforms = ["linux/amd64", "linux/arm64", "linux/arm/v7"]
}