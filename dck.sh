#!/bin/sh

if [ "$1" = "build" ]; then
  sudo docker build -t bike_backend .
elif [ "$1" = "run" ]; then
  if [ ! "$2" = ""  ]; then
    for i in `seq 1 $2`; do
      port=$(( 35009 + $i ))
      echo "Running server (port map: $port -> 35010)"
      sudo docker run -d --rm -p $port:35010 --name="bike-backend-$port" bike_backend ./Backend/target/release/bike_blockchain_backend
    done
  else
    echo "Running single server"
    sudo docker run -d --rm -p 35010:35010 --name="bike-backend" bike_backend cd Backend && cargo run
  fi
else 
  echo Invalid
fi
