FROM node:latest

WORKDIR /app

RUN npm install express
RUN npm install ejs

COPY ./server.js /app
COPY ./public /app/public
COPY ./views /app/views

COPY ./docker-entrypoint.sh /root

CMD ["/bin/bash", "/root/docker-entrypoint.sh"]
