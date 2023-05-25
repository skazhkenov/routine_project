$(document).ready(async function() {

    if (theCookieExist('x-auth')) {

        async function logOut(auth_token) {
            showOverlay();
            let out = await fetch('/logout', {
                method: 'DELETE',
                headers: {
                    'Content-Type': 'application/json;charset=utf-8', 
                    'Authorization': 'Bearer ' + auth_token
                }
            });
            hideOverlay();
            window.location.href = '/';
        }

        let token = getCookieValue('x-auth');
        let check_login = await fetch('/get_user', {
            method: 'GET',
            headers: {
                'Content-Type': 'application/json;charset=utf-8', 
                'Authorization': 'Bearer ' + token
            }
        });

        var profileData = {
            name: "",
            email: ""
        };

        let check_login_status = check_login.status; 
        let user_data;

        if (check_login_status == 200) {
            console.log('Ok');
            user_data = await check_login.json();
            profileData.name = user_data[0].name;
            profileData.email = user_data[0].email;
        } else {
            window.location.href = '/';
        };

        const nameElement = document.getElementById('profile-name');
        const emailElement = document.getElementById('profile-email');

        nameElement.textContent = profileData.name;
        emailElement.textContent = profileData.email;

        const changeNameBtn = document.getElementById('change-name-btn');
        const changeEmailBtn = document.getElementById('change-email-btn');
        const changePasswordBtn = document.getElementById('change-password-btn');
        const logoutBtn = document.getElementById('logout');

        const changeEmailPopup = document.getElementById('change-email-popup');
        const changePasswordPopup = document.getElementById('change-password-popup');

        changeNameBtn.addEventListener('click', async function() {
            
            $("#changeNameModal").css("display", "block");
            $("#change-name-popup").css("display", "block");
            

            $(".close").click(async function() {
                $(".popup").css("display", "none");
                $(".modal").css("display", "none");
            });

            $(window).click(async function(event) {
                if (event.target == $("#changeNameModal")[0]) {
                    $(".popup").css("display", "none");
                    $(".modal").css("display", "none");
                }
            });
        });

        changeEmailBtn.addEventListener('click', async function() {
            $("#changeEmailModal").css("display", "block");
            $("#change-email-popup").css("display", "block");


            $(".close").click(async function() {
                $(".popup").css("display", "none");
                $(".modal").css("display", "none");
            });

            $(window).click(async function(event) {
                if (event.target == $("#changeEmailModal")[0]) {
                    $(".popup").css("display", "none");
                    $(".modal").css("display", "none");
                }
            });
        });

        changePasswordBtn.addEventListener('click', async function() {
            $("#changePasswordModal").css("display", "block");
            $("#change-password-popup").css("display", "block");

            $(".close").click(async function() {
                $(".popup").css("display", "none");
                $(".modal").css("display", "none");
            });

            $(window).click(async function(event) {
                if (event.target == $("#changePasswordModal")[0]) {
                    $(".popup").css("display", "none");
                    $(".modal").css("display", "none");
                }
            });
        });

        const changeNameForm = document.getElementById('change-name-form');
        const changeEmailForm = document.getElementById('change-email-form');
        const changePasswordForm = document.getElementById('change-password-form');

        changeNameForm.addEventListener('submit', async function(e) {
            e.preventDefault();
            const newNameInput = document.getElementById('new-name-input');
            const newName = newNameInput.value;

            let changeNameRequestBody = {
                "new_name": newName
            };

            showOverlay();
            let changeNameRequest = await fetch('/change_username', {
                method: 'PUT',
                headers: {
                    'Content-Type': 'application/json;charset=utf-8', 
                    'Authorization': 'Bearer ' + token
                }, 
                body: JSON.stringify(changeNameRequestBody)
            });
            let changeNameRequestStatus = changeNameRequest.status;
            console.log(changeNameRequestStatus);
            if (changeNameRequestStatus == 200) {
                hideOverlay();

                $(".popup").css("display", "none");
                $(".modal").css("display", "none");
                newNameInput.value = '';
                nameElement.textContent = newName;

            } else {
                hideOverlay();
                alert("Something goes wrong.\nPlease try later.")
            }
            
        });

        changeEmailForm.addEventListener('submit', async function(e) {
            e.preventDefault();
            const newEmailInput = document.getElementById('new-email-input');
            const newEmail = newEmailInput.value;

            let changeEmailRequestBody = {
                "new_email": newEmail
            };

            showOverlay();
            let changeEmailRequest = await fetch('/change_email', {
                method: 'PUT',
                headers: {
                    'Content-Type': 'application/json;charset=utf-8', 
                    'Authorization': 'Bearer ' + token
                }, 
                body: JSON.stringify(changeEmailRequestBody)
            });
            let changeEmailRequestStatus = changeEmailRequest.status;
            console.log(changeEmailRequestStatus);
            if (changeEmailRequestStatus == 200) {
                hideOverlay();

                $(".popup").css("display", "none");
                $(".modal").css("display", "none");

                newEmailInput.value = '';

            } else if (changeEmailRequestStatus == 400) {
                let response = await changeEmailRequest.json();
                let message = response['message'];
                hideOverlay();
                alert(message)
            } else {
                hideOverlay();
                alert("Something goes wrong.\nPlease try later.")
            }

            
        });

        changePasswordForm.addEventListener('submit', async function(e) {
            e.preventDefault();

            const oldPasswordInput = document.getElementById('old-password-input');
            const newPasswordInput = document.getElementById('new-password-input');
            const repeatPasswordInput = document.getElementById('new-password-input2');

            const oldPassword = oldPasswordInput.value;
            const newPassword = newPasswordInput.value;
            const repeatPassword = repeatPasswordInput.value;

            if ((newPassword.length >= 10) && (newPassword.length <= 64)) {

                if (newPassword == repeatPassword) {
                    if (oldPassword != newPassword) {
                
                        let changePassRequestBody = {
                            "old_password": oldPassword,
                            "new_password": newPassword
                        };

                        showOverlay();
                        let changePassRequest = await fetch('/change_password', {
                            method: 'PUT',
                            headers: {
                                'Content-Type': 'application/json;charset=utf-8', 
                                'Authorization': 'Bearer ' + token
                            }, 
                            body: JSON.stringify(changePassRequestBody)
                        });

                        let changePassRequestStatus = changePassRequest.status;
                        console.log(changePassRequestStatus);
                        if (changePassRequestStatus == 200) {

                            hideOverlay();
                            alert("Password successfully changed.");

                            $(".popup").css("display", "none");
                            $(".modal").css("display", "none");

                            
                            oldPasswordInput.value = '';
                            newPasswordInput.value = '';
                            repeatPasswordInput.value = '';

                            await logOut(token);

                        } else {
                            hideOverlay();
                            alert("Something goes wrong.\nPlease try later.")
                        }
                        

                    } else {
                        alert("New password couldn't be the same as current!");
                        newPasswordInput.value = '';
                        repeatPasswordInput.value = '';
                    }
                    
                } else {
                    
                    alert("Repeated password is not equal to new password!");
                    newPasswordInput.value = '';
                    repeatPasswordInput.value = '';
                }
            } else {
                if (newPassword.length < 10) {
                    alert("Password length couldn't be less than 10 signs");
                } else {
                    alert("Password length couldn't be greater than 64 signs");
                }
            }
        });


        jQuery('#logout').on('click', async function(){
            await logOut(token);
        });
    

    } else {    
        window.location.href = '/';
    }
});


function theCookieExist(cookieName) {
    var cookies = document.cookie.split(';');
  
    for (var i = 0; i < cookies.length; i++) {
      var cookie = cookies[i].trim();
  
      if (cookie.startsWith(cookieName + '=')) {
        return true;
      }
    }
  
    return false;
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