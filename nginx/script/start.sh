rm -f /etc/nginx/sites-enabled/default

cp /etc/nginx/sites-available/dev-home-project-r001.site.conf /etc/nginx/conf.d
service nginx start
