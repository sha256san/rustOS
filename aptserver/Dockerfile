FROM python:3.11-slim

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    tar \
    xz-utils \
    && rm -rf /var/lib/apt/lists/*

COPY . /app

EXPOSE 8080 12345

CMD ["python3", "server.py"]
