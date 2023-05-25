FROM ubuntu:18.04

RUN apt update
RUN apt install -y vim htop tmux nginx

RUN export DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y tzdata 
RUN ln -fs /usr/share/zoneinfo/America/New_York /etc/localtime 
RUN dpkg-reconfigure --frontend noninteractive tzdata

RUN apt install -y letsencrypt

RUN apt-get -y update
RUN apt-get -y dist-upgrade
RUN apt-get -y install software-properties-common
RUN add-apt-repository ppa:certbot/certbot
RUN apt install -y python-certbot-nginx

COPY ./docker-entrypoint.sh /root

CMD ["/bin/sh", "/root/docker-entrypoint.sh"]
