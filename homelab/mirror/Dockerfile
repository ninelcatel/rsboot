FROM nginx:alpine
RUN apk add python3 py3-requests 
# COPY ./distros.json /home/distros.json
# COPY ./update_distros.py /home/update_distros.py
CMD ["sh", "-c", "python3 /home/update_distros.py /home/distros.json /usr/share/nginx/html/os_list & exec nginx -g 'daemon off;'"]
