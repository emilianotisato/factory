<?php

namespace App\Services;

use App\Models\User;

class OtherService implements Notifier
{
    public function welcome(User $user): void
    {
        $this->notify($user, 'welcome');
    }

    public function notify(User $user, string $template): void
    {
        mail($user->email, $template, '');
    }
}
