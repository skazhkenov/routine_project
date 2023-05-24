jQuery('document').ready(async function(){

  function timeSleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
  }

  function getCookieValue(cookieName) {
    const cookie = document.cookie.match('(^|;)\\s*' + cookieName + '\\s*=\\s*([^;]+)');
    return cookie ? cookie.pop() : '';
  }

  function showOverlay() {
      document.getElementById("overlay").style.display = "flex";
  }
    
  function hideOverlay() {
      document.getElementById("overlay").style.display = "none";
  }

  let check_authorisation_request = await fetch('/user_boards', {
    method: 'GET',
    headers: {
        'Content-Type': 'application/json;charset=utf-8', 
        'Authorization': 'Bearer ' + getCookieValue('x-auth')
    }
  });

  let check_authorisation_result = check_authorisation_request.status; 
  if (check_authorisation_result == 200) {
    window.location.replace("/boards");  
  }

  jQuery('document').ready( async function(){

    // login
    const loginForm = document.getElementById('loginForm'); 
    const loginInput = document.getElementById('username');
    const passwordInput = document.getElementById('password');
    
    jQuery('#sign-button').on('click', async function(){

      var username;
      username = jQuery('#username').val();

      var password;
      password = jQuery('#password').val();

      var user_data = {
        "email": username,
        "password": password
      };

      showOverlay();
      let user_data_value = await fetch('/authorization', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json;charset=utf-8'
        },
        body: JSON.stringify(user_data)

      });
      let login_result = user_data_value.status; 
      console.log(login_result);

      if (login_result == 200) {

        hideOverlay();
        window.location.replace("/boards");  
      } else {
        
        alert('Unexpected issue happened. \nPlease try later.');
        hideOverlay();
      }
      
      
    });

    $(".forgot-password").click(async function() {
      $("#forgotPasswordModal").css("display", "block");
    });
  
    $(".close").click(async function() {
      $("#forgotPasswordModal").css("display", "none");
    });
  
    $(window).click(async function(event) {
      if (event.target == $("#forgotPasswordModal")[0]) {
        $("#forgotPasswordModal").css("display", "none");
      }
    });
  
    $("#forgotPasswordForm").submit(async function(event) {
      event.preventDefault();
  
      showOverlay();

      var email = $("#email").val();
      console.log(email);

      let forgot_password_request = await fetch('/forgot_password', {
        method: 'PUT',
        headers: {
            'Content-Type': 'application/json;charset=utf-8'
        },
        body: JSON.stringify({
          "email": email
        })
      });

      let forgot_password_status = forgot_password_request.status;

      console.log(forgot_password_status);
      if (forgot_password_status == 200) {
        hideOverlay();
        alert(`Temporary password sent to email address: ${email}`);
      } else {
        hideOverlay();
        alert('Out of service. Please try later.');
      }

      $("#forgotPasswordModal").css("display", "none");
    });

    jQuery('#register-button').on('click', async function(){
      $("#registerModal").css("display", "block");
    });
  
    $(".close").click(function() {
      $("#registerModal").css("display", "none");
    });
  
    $(window).click(function(event) {
      if (event.target == $("#registerModal")[0]) {
        $("#registerModal").css("display", "none");
      }
    });
  
    $("#registrationForm").submit(async function(event) {
      event.preventDefault();
  
      var username = $("#reg_username").val();
      var password = $("#reg_password").val();
      var email = $("#reg_email").val();

      jQuery('#registration-submit').on('click', async function(){

        if ((password.length >= 10) && (password.length <= 64)) {

          showOverlay();
          let new_user_credentials = {
              "name": username,
              "email": email,
              "password": password
          };
          console.log(new_user_credentials);
          let user_registration_result = await fetch('/create_user', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json;charset=utf-8'
            },
            body: JSON.stringify(new_user_credentials)
          });
    
          let user_registration_status = user_registration_result.status;
          if (user_registration_status == 200) {
            hideOverlay();
            alert("You've successfully registrated.\nCheck verification message, we've sent to \nyour email and finish your authentification.");
            
            $("#registerModal").css("display", "none");
          } else if (user_registration_status == 400) {

            hideOverlay();
            let message = "Invalid credentials";
            console.log(user_registration_status, user_registration_result.body);
            alert(message);
          } else {
            hideOverlay();
            alert("Unexpected issue happened. \nPlease try later.");
          } 

        } else {
          if (password.length < 10) {
            alert("Password length couldn't be less than 10 signs");
          } else {
            alert("Password length couldn't be greater than 64 signs");
          }
        }
      });

    });
  });
});
